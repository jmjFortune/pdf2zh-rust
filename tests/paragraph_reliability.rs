use anyhow::Result;
use axum::{Json, Router, http::StatusCode, routing::post};
use mupdf::{
    Point,
    pdf::PdfDocument,
    shape::{Shape, TextOptions},
};
use pdf2zh_rust::{
    engine::{self, JobConfig},
    pdf,
    translate::TranslationSettings,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

fn fixture(path: &std::path::Path) -> Result<()> {
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page((612., 792.))?;
    let mut shape = Shape::new(&mut page)?;
    for (y, text) in [
        (100., "Success paragraph translates normally."),
        (
            280.,
            "Overflow paragraph must remain readable in the original PDF.",
        ),
        (460., "Marker paragraph preserves inline code "),
    ] {
        shape.insert_text(
            Point::new(55., y),
            text,
            &TextOptions {
                fontsize: 11.,
                fontname: "Times-Roman".into(),
                ..Default::default()
            },
        )?;
    }
    shape.insert_text(
        Point::new(265., 460.),
        "variable",
        &TextOptions {
            fontsize: 11.,
            fontname: "Courier".into(),
            ..Default::default()
        },
    )?;
    shape.commit(&mut doc, true)?;
    pdf::save(&doc, path)
}

struct Mock {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Mock {
    async fn start(status: StatusCode, mode: &'static str) -> Result<Self> {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let app = Router::new().route(
            "/chat/completions",
            post(move |Json(body): Json<Value>| {
                let text = body["messages"][1]["content"].as_str().unwrap().to_owned();
                seen.lock().unwrap().push(text.clone());
                async move {
                    if !status.is_success() {
                        return (status, Json(json!({"error":{"code":"invalid_api_key"}})));
                    }
                    let output = if mode == "all_overflow" || text.contains("Overflow") {
                        "译".repeat(1000)
                    } else if mode == "formula_overflow" && text.contains("Marker") {
                        format!("{} ⟦0⟧", "译".repeat(1000))
                    } else if text.contains("Marker") {
                        "公式标记缺失。".into()
                    } else {
                        "成功段落。".into()
                    };
                    (
                        status,
                        Json(json!({"choices":[{"message":{"content":output}}]})),
                    )
                }
            }),
        );
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

fn config(root: &std::path::Path, mock: &Mock) -> JobConfig {
    JobConfig {
        input: root.join("input.pdf"),
        output: root.join("output.pdf"),
        data_dir: root.join("data"),
        layout: false,
        translation: TranslationSettings {
            provider: "openai".into(),
            base_url: mock.url.clone(),
            model: "local-contract".into(),
            api_key: Some("local-test".into()),
            requests_per_second: 100.,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn partial_translation_preserves_failed_source_and_reports_each_paragraph() -> Result<()> {
    verify_partial("normal").await
}

#[tokio::test(flavor = "current_thread")]
async fn formula_layout_overflow_also_preserves_original_pixels() -> Result<()> {
    verify_partial("formula_overflow").await
}

async fn verify_partial(mode: &'static str) -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let mock = Mock::start(StatusCode::OK, mode).await?;
    let mut config = config(tmp.path(), &mock);
    fixture(&config.input)?;
    let mut events = Vec::new();
    engine::run(&config, |event| events.push(event)).await?;
    let output = pdf::open(&config.output, None)?;
    let text = pdf::page_text(&output, 0)?
        .blocks
        .iter()
        .map(|b| b.text())
        .collect::<String>();
    assert!(text.contains("成功段落"));
    assert!(text.contains("Overflow paragraph must remain readable"));
    assert!(text.contains("Marker paragraph preserves inline code"));
    assert!(text.contains("variable"));
    let source = pdf::open(&config.input, None)?;
    let original_pixels = pdf::render_rgb(&source.load_pdf_page(0)?, 1.)?;
    let output_pixels = pdf::render_rgb(&output.load_pdf_page(0)?, 1.)?;
    assert_eq!(
        image::imageops::crop_imm(&original_pixels, 0, 200, 612, 400).to_image(),
        image::imageops::crop_imm(&output_pixels, 0, 200, 612, 400).to_image(),
        "failed paragraphs and their inline code must remain pixel-identical"
    );
    let warnings: Value =
        serde_json::from_slice(&std::fs::read(config.data_dir.join("warnings.json"))?)?;
    assert_eq!(warnings.as_array().unwrap().len(), 2);
    assert_eq!(warnings[0]["page"], 1);
    assert_eq!(warnings[0]["paragraph"], 2);
    assert_eq!(warnings[1]["paragraph"], 3);
    assert!(
        events
            .last()
            .unwrap()
            .message
            .contains("2 paragraphs retained")
    );
    assert_eq!(events.last().unwrap().completed_blocks, 1);
    assert_eq!(
        mock.requests.lock().unwrap().len(),
        if mode == "normal" { 5 } else { 3 }
    );
    // Successful output and overlong-but-valid text are cached; invalid markers are not.
    config.output = tmp.path().join("rerun.pdf");
    engine::run(&config, |_| {}).await?;
    assert_eq!(
        mock.requests.lock().unwrap().len(),
        if mode == "normal" { 8 } else { 3 }
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn authentication_failure_and_cancellation_still_fail_without_output() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let mock = Mock::start(StatusCode::UNAUTHORIZED, "normal").await?;
    let config = config(tmp.path(), &mock);
    fixture(&config.input)?;
    let error = engine::run(&config, |_| {}).await.unwrap_err();
    assert!(format!("{error:#}").contains("HTTP 401"));
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
    assert!(!config.output.exists());
    std::fs::write(config.data_dir.join("cancel"), "")?;
    let error = engine::run(&config, |_| {}).await.unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(!config.output.exists());
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn all_paragraphs_failing_cannot_be_reported_as_success() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let mock = Mock::start(StatusCode::OK, "all_overflow").await?;
    let config = config(tmp.path(), &mock);
    fixture(&config.input)?;
    let error = engine::run(&config, |_| {}).await.unwrap_err();
    assert!(error.to_string().contains("no paragraphs translated"));
    assert!(!config.output.exists());
    Ok(())
}
