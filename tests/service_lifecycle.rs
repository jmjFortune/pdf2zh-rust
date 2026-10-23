use anyhow::{Result, bail};
use pdf2zh_rust::{pdf, service::JobInfo};
use serde_json::json;
use std::{
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn poll(client: &reqwest::Client, base: &str, id: &str, terminal: bool) -> Result<JobInfo> {
    for _ in 0..150 {
        let job: JobInfo = client
            .get(format!("{base}/api/jobs/{id}"))
            .send()
            .await?
            .json()
            .await?;
        let done =
            ["completed", "failed", "cancelled", "interrupted"].contains(&job.status.as_str());
        if (terminal && done) || (!terminal && job.status == "running") {
            return Ok(job);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("task did not reach expected state")
}

async fn upload(
    client: &reqwest::Client,
    base: &str,
    pdf: &[u8],
    mock: &str,
    layout: bool,
) -> Result<JobInfo> {
    let options = json!({"layout":layout,"translation":{"provider":"openai","source":"en","target":"zh-CN","base_url":mock,"model":format!("contract-test-{}",uuid::Uuid::new_v4())}});
    let boundary = "RustLifecycleBoundary";
    let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{options}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
    body.extend(pdf);
    body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    let response = client
        .post(format!("{base}/api/jobs"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await?;
    anyhow::ensure!(response.status().is_success(), "upload failed");
    Ok(response.json().await?)
}

/// Offline HTTP contract and process-lifecycle test. The mock translator is
/// deliberate; live Google/model/PDF checks are separate manual evidence.
#[tokio::test(flavor = "current_thread")]
async fn upload_download_failure_and_cancellation_use_real_worker_processes() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("fixture.pdf");
    pdf::fixture(&input)?;
    let bytes = std::fs::read(&input)?;
    let delay = Arc::new(AtomicU64::new(0));
    let profiling = std::env::var_os("PDF2ZH_PROFILE_MEMORY").is_some();
    if profiling {
        delay.store(3000, Ordering::Relaxed);
    }
    let handler_delay = delay.clone();
    let requests = Arc::new(AtomicU64::new(0));
    let handler_requests = requests.clone();
    let mock = axum::Router::new().route(
        "/chat/completions",
        axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let delay = handler_delay.clone();
            let requests = handler_requests.clone();
            async move {
                requests.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(delay.load(Ordering::Relaxed))).await;
                let text = if body["messages"][1]["content"]
                    .as_str()
                    .unwrap_or("")
                    .contains('⟦')
                {
                    "编码器由⟦0⟧组成。"
                } else {
                    "你好，世界。这是契约测试译文。"
                };
                axum::Json(json!({"choices":[{"message":{"content":text}}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mock_url = format!("http://{}", listener.local_addr()?);
    let mock_handle = tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });
    let port = std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port();
    let base = format!("http://127.0.0.1:{port}");
    let data = tmp.path().join("data");
    let mut process = Command::new(env!("CARGO_BIN_EXE_pdf2zh-rust"));
    process
        .arg("serve")
        .arg("--listen")
        .arg(format!("127.0.0.1:{port}"))
        .arg("--data-dir")
        .arg(&data)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let server = Server(process.spawn()?);
    let stop_profile = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let profiler = if profiling {
        let stop = stop_profile.clone();
        let requests = requests.clone();
        let root = server.0.id();
        Some(tokio::spawn(async move {
            let start = std::time::Instant::now();
            let mut samples = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                if let Ok(output) = Command::new("ps")
                    .args(["-axo", "pid=,ppid=,rss="])
                    .output()
                {
                    let rows = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter_map(|line| {
                            let n = line
                                .split_whitespace()
                                .filter_map(|s| s.parse::<u64>().ok())
                                .collect::<Vec<_>>();
                            (n.len() == 3).then(|| (n[0], n[1], n[2]))
                        })
                        .collect::<Vec<_>>();
                    let resident = rows
                        .iter()
                        .find(|r| r.0 == root as u64)
                        .map(|r| r.2)
                        .unwrap_or(0);
                    let workers = rows
                        .iter()
                        .filter(|r| r.1 == root as u64)
                        .map(|r| r.0)
                        .collect::<Vec<_>>();
                    let worker_rss = rows
                        .iter()
                        .filter(|r| workers.contains(&r.0))
                        .map(|r| r.2)
                        .sum::<u64>();
                    let analyzer_rss = rows
                        .iter()
                        .filter(|r| workers.contains(&r.1))
                        .map(|r| r.2)
                        .sum::<u64>();
                    samples.push(json!({"ms":start.elapsed().as_millis(),"service_kib":resident,"worker_kib":worker_rss,"analyzer_kib":analyzer_rss,"translation_requests":requests.load(Ordering::Relaxed)}));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            samples
        }))
    } else {
        None
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    for attempt in 0..100 {
        if client
            .get(format!("{base}/api/health"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        anyhow::ensure!(attempt < 99, "service failed to start");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let first = upload(&client, &base, &bytes, &mock_url, true).await?;
    let first = poll(&client, &base, &first.id, true).await?;
    assert_eq!(first.status, "completed", "{:?}", first.error);
    assert_eq!(first.output_pages, Some(1));
    let preview = client
        .get(format!("{base}/api/jobs/{}/preview/1", first.id))
        .send()
        .await?
        .bytes()
        .await?;
    assert!(preview.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(
        !data.join(&first.id).join("job.json").exists(),
        "private settings snapshot must be removed"
    );
    let result = client
        .get(format!("{base}/api/jobs/{}/output", first.id))
        .send()
        .await?
        .bytes()
        .await?;
    {
        let doc = mupdf::pdf::PdfDocument::from_bytes(&result)?;
        let text = pdf::page_text(&doc, 0)?
            .blocks
            .iter()
            .map(|b| b.text())
            .collect::<String>();
        assert!(text.contains("你好"));
        assert!(text.contains("E = mc2"));
    }
    let rich_input = tmp.path().join("inline.pdf");
    {
        use mupdf::{
            Point, Size,
            pdf::PdfDocument,
            shape::{Shape, TextOptions},
        };
        let mut doc = PdfDocument::new();
        let mut page = doc.new_page(Size::A4)?;
        let mut shape = Shape::new(&mut page)?;
        shape.insert_text(
            Point::new(72., 100.),
            "The encoder uses ",
            &TextOptions::default(),
        )?;
        shape.insert_text(
            Point::new(160., 100.),
            "α",
            &TextOptions {
                fontname: "Symbol".into(),
                simple: false,
                ..Default::default()
            },
        )?;
        shape.insert_text(
            Point::new(170., 100.),
            " identical layers.",
            &TextOptions::default(),
        )?;
        shape.commit(&mut doc, true)?;
        doc.save(rich_input.to_str().unwrap())?;
    }
    let source_text = {
        let doc = pdf::open(&rich_input, None)?;
        pdf::page_text(&doc, 0)?
            .blocks
            .iter()
            .map(|b| b.text())
            .collect::<String>()
    };
    assert!(
        source_text.contains('α'),
        "source Greek fixture is invalid: {source_text}"
    );
    let rich = upload(
        &client,
        &base,
        &std::fs::read(&rich_input)?,
        &mock_url,
        false,
    )
    .await?;
    let rich = poll(&client, &base, &rich.id, true).await?;
    assert_eq!(rich.status, "completed", "{:?}", rich.error);
    {
        let doc = pdf::open(&data.join(&rich.id).join("output.pdf"), None)?;
        let text = pdf::page_text(&doc, 0)?
            .blocks
            .iter()
            .map(|b| b.text())
            .collect::<String>();
        assert!(text.contains("编码器"), "{text}");
        let before = text.find('由').unwrap();
        let formula = text
            .find('α')
            .unwrap_or_else(|| panic!("Greek formula lost. Source: {source_text}; output: {text}"));
        let after = text.find("组成").unwrap();
        assert!(
            before < formula && formula < after,
            "formula reading order is incorrect: {text}"
        );
    }
    let invalid = upload(&client, &base, b"not a PDF", &mock_url, false).await?;
    let invalid = poll(&client, &base, &invalid.id, true).await?;
    assert_eq!(invalid.status, "failed");
    assert!(!data.join(&invalid.id).join("output.pdf").exists());
    delay.store(20_000, Ordering::Relaxed);
    let expected_request = requests.load(Ordering::Relaxed) + 1;
    let running = upload(&client, &base, &bytes, &mock_url, false).await?;
    poll(&client, &base, &running.id, false).await?;
    for attempt in 0..100 {
        if requests.load(Ordering::Relaxed) >= expected_request {
            break;
        }
        anyhow::ensure!(
            attempt < 99,
            "worker did not reach blocking translation request"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let queued = upload(&client, &base, &bytes, &mock_url, false).await?;
    for id in [&queued.id, &running.id] {
        client
            .post(format!("{base}/api/jobs/{id}/cancel"))
            .send()
            .await?
            .error_for_status()?;
        let result = poll(&client, &base, id, true).await?;
        assert_eq!(result.status, "cancelled");
        assert!(!data.join(id).join("output.pdf").exists());
    }
    let jobs: Vec<JobInfo> = client
        .get(format!("{base}/api/jobs"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(jobs.len(), 5);
    drop(server);
    {
        let db = rusqlite::Connection::open(data.join("jobs.sqlite3"))?;
        db.execute("UPDATE jobs SET status='running' WHERE id=?1", [&first.id])?;
    }
    let restarted = Server(process.spawn()?);
    for attempt in 0..100 {
        if client
            .get(format!("{base}/api/health"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        anyhow::ensure!(attempt < 99, "service failed to restart");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let recovered = poll(&client, &base, &first.id, true).await?;
    assert_eq!(recovered.status, "interrupted");
    drop(restarted);
    stop_profile.store(true, Ordering::Relaxed);
    if let Some(profiler) = profiler {
        let samples = profiler.await?;
        std::fs::create_dir_all("artifacts")?;
        std::fs::write(
            "artifacts/native-stage-memory.json",
            serde_json::to_vec_pretty(&samples)?,
        )?;
    }
    mock_handle.abort();
    Ok(())
}
