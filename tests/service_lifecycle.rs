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

async fn upload(client: &reqwest::Client, base: &str, pdf: &[u8], mock: &str) -> Result<JobInfo> {
    let options = json!({"layout":false,"translation":{"provider":"openai","source":"en","target":"zh-CN","base_url":mock,"model":format!("contract-test-{}",uuid::Uuid::new_v4())}});
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
    let handler_delay = delay.clone();
    let requests = Arc::new(AtomicU64::new(0));
    let handler_requests = requests.clone();
    let mock = axum::Router::new().route(
        "/chat/completions",
        axum::routing::post(move || {
            let delay = handler_delay.clone();
            let requests = handler_requests.clone();
            async move {
                requests.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(delay.load(Ordering::Relaxed))).await;
                axum::Json(
                    json!({"choices":[{"message":{"content":"你好，世界。这是契约测试译文。"}}]}),
                )
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
    let first = upload(&client, &base, &bytes, &mock_url).await?;
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
    let invalid = upload(&client, &base, b"not a PDF", &mock_url).await?;
    let invalid = poll(&client, &base, &invalid.id, true).await?;
    assert_eq!(invalid.status, "failed");
    assert!(!data.join(&invalid.id).join("output.pdf").exists());
    delay.store(20_000, Ordering::Relaxed);
    let running = upload(&client, &base, &bytes, &mock_url).await?;
    poll(&client, &base, &running.id, false).await?;
    for attempt in 0..100 {
        if requests.load(Ordering::Relaxed) >= 2 {
            break;
        }
        anyhow::ensure!(
            attempt < 99,
            "worker did not reach blocking translation request"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let queued = upload(&client, &base, &bytes, &mock_url).await?;
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
    assert_eq!(jobs.len(), 4);
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
    mock_handle.abort();
    Ok(())
}
