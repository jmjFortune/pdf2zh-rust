use anyhow::Result;
use axum::{Json, Router, routing::post};
use pdf2zh_rust::{pdf, translate::TranslationSettings};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

fn fixture(path: &Path, pages: usize) -> Result<()> {
    use mupdf::{
        Point, Rect,
        pdf::PdfDocument,
        shape::{Shape, TextOptions, TextboxOptions},
    };
    let mut doc = PdfDocument::new();
    for index in 0..pages {
        let mut page = doc.new_page((612., 792.))?;
        let mut shape = Shape::new(&mut page)?;
        shape.insert_text(
            Point::new(55., 35.),
            "Cross-page diagnostic",
            &TextOptions {
                fontsize: 9.,
                ..Default::default()
            },
        )?;
        for (y, text) in [
            (
                85.,
                "The evaluation uses held-out examples that are never shown during training. We measure average error and variation across independent runs.",
            ),
            (
                205.,
                "All settings are recorded before evaluation begins. Both methods receive the same input and are evaluated using identical metrics.",
            ),
            (
                325.,
                "The training procedure and the evaluation settings remain fixed. This makes the comparison reproducible across different experimental runs.",
            ),
            (
                445.,
                "We report the measured results together with uncertainty. A lower numerical value alone does not establish a meaningful improvement.",
            ),
            (
                630.,
                "We tested both methods under identical conditions with independent random seeds. The difference was not statistically",
            ),
        ] {
            let text = if index > 0 && y == 85. {
                "significant (p = 0.12). This conclusion applies to the full set of repeated trials rather than only to the best individual run."
            } else if index > 0 && y == 630. {
                "The final review includes all repeated trials and confirms that no observations were removed from the evaluation."
            } else {
                text
            };
            assert!(
                shape.insert_textbox(
                    Rect::new(55., y, 557., y + 80.),
                    text,
                    &TextboxOptions {
                        fontsize: 11.,
                        fontname: "Times-Roman".into(),
                        ..Default::default()
                    }
                )? >= 0.
            );
        }
        shape.insert_text(
            Point::new(303., 770.),
            &(index + 1).to_string(),
            &TextOptions {
                fontsize: 10.,
                ..Default::default()
            },
        )?;
        shape.commit(&mut doc, true)?;
        if index % 2 == 1 {
            page.set_rotation(90)?;
        }
    }
    pdf::save(&doc, path)
}

struct Mock {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Mock {
    async fn start() -> Result<Self> {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let app = Router::new().route("/chat/completions", post(move |Json(body): Json<Value>| {
            captured.lock().unwrap().push(body.clone());
            async move {
                let input = body["messages"][1]["content"].as_str().unwrap();
                let translated = |text: &str| if text.contains("not statistically") { "前页未完句。" } else if text.contains("significant (p") { "后页续句。" } else { "背景说明。" };
                let output = if let Ok(items) = serde_json::from_str::<Vec<Value>>(input) {
                    // IDs, rather than response order, must determine page ownership.
                    serde_json::to_string(&items.iter().rev().map(|item| json!({"id":item["id"],"output":translated(item["input"].as_str().unwrap())})).collect::<Vec<_>>()).unwrap()
                } else { translated(input).to_owned() };
                Json(json!({"choices":[{"message":{"content":output}}]}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Ok(Self {
            url,
            requests,
            task,
        })
    }
}

async fn translate(
    input: &Path,
    output: &Path,
    data: &Path,
    mock: &Mock,
    pages: &str,
) -> Result<()> {
    let config = data.with_extension("json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({
            "translation": TranslationSettings { provider:"openai".into(), base_url:mock.url.clone(), model:"cross-page-contract".into(), requests_per_second:100., ..Default::default() },
            "layout":true,"pages":pages,"data_dir":data,"format":"mono"
        }))?,
    )?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_pdf2zh-rust"))
        .arg("translate")
        .arg(input)
        .arg(output)
        .arg("--config")
        .arg(config)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("PDF2ZH_API_KEY")
        .env_remove("PDF2ZH_CACHE_ONLY")
        .output()
        .await?;
    anyhow::ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn adjacent_body_uses_one_batch_and_keeps_each_translation_on_its_page() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("input.pdf");
    fixture(&input, 2)?;
    let mock = Mock::start().await?;
    let output = tmp.path().join("output.pdf");
    let data = tmp.path().join("data");
    translate(&input, &output, &data, &mock, "").await?;
    let batches = mock
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|r| {
            serde_json::from_str::<Vec<Value>>(r["messages"][1]["content"].as_str().unwrap()).ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        batches.len(),
        1,
        "adjacent-page body must share exactly one LLM request"
    );
    assert!(
        batches[0][0]["input"]
            .as_str()
            .unwrap()
            .contains("not statistically")
    );
    assert!(
        batches[0][1]["input"]
            .as_str()
            .unwrap()
            .contains("significant (p")
    );
    let doc = pdf::open(&output, None)?;
    assert_eq!(doc.page_count()?, 2);
    assert_eq!(
        doc.load_pdf_page(1)?.rotation()?,
        90,
        "lookahead must preserve page rotation"
    );
    let page_text = |page| {
        pdf::page_text(&doc, page)
            .unwrap()
            .blocks
            .iter()
            .map(|b| b.text())
            .collect::<String>()
    };
    assert!(
        page_text(0).contains("前页未完句") && !page_text(0).contains("后页续句"),
        "{}",
        page_text(0)
    );
    assert!(
        page_text(1).contains("后页续句") && !page_text(1).contains("前页未完句"),
        "{}",
        page_text(1)
    );
    assert!(
        !page_text(0).contains("not statistically") && !page_text(1).contains("significant (p")
    );
    let calls = mock.requests.lock().unwrap().len();
    translate(&input, &tmp.path().join("cached.pdf"), &data, &mock, "").await?;
    assert_eq!(
        mock.requests.lock().unwrap().len(),
        calls,
        "same boundary must reuse its contextual cache"
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn skipped_pages_are_not_paired() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("input.pdf");
    fixture(&input, 3)?;
    let mock = Mock::start().await?;
    translate(
        &input,
        &tmp.path().join("output.pdf"),
        &tmp.path().join("data"),
        &mock,
        "1,3",
    )
    .await?;
    assert!(mock.requests.lock().unwrap().iter().all(|r| {
        serde_json::from_str::<Vec<Value>>(r["messages"][1]["content"].as_str().unwrap()).is_err()
    }));
    let output = pdf::open(&tmp.path().join("output.pdf"), None)?;
    let untouched = pdf::page_text(&output, 1)?
        .blocks
        .iter()
        .map(|b| b.text())
        .collect::<String>();
    assert!(
        untouched.contains("significant"),
        "unselected page must remain original"
    );
    Ok(())
}
