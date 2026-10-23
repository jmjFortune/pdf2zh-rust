use anyhow::{Result, ensure};
use pdf2zh_rust::pdf;
use serde_json::{Value, json};
use std::{
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
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
async fn start(data: &std::path::Path, web: &std::path::Path) -> Result<(Server, String)> {
    let port = std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port();
    let base = format!("http://127.0.0.1:{port}");
    let process = Command::new(env!("CARGO_BIN_EXE_pdf2zh-rust"))
        .args([
            "serve",
            "--listen",
            &format!("127.0.0.1:{port}"),
            "--data-dir",
        ])
        .arg(data)
        .env("PDF2ZH_WEB_DIR", web)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let server = Server(process);
    let c = reqwest::Client::new();
    for attempt in 0..100 {
        if c.get(format!("{base}/api/health")).send().await.is_ok() {
            return Ok((server, base));
        }
        ensure!(attempt < 99, "server failed");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    unreachable!()
}
fn multipart(pdf: Option<&[u8]>) -> Vec<u8> {
    let mut body = Vec::new();
    for (key, value) in [
        ("service", "openai"),
        ("lang_in", "en"),
        ("lang_out", "zh-CN"),
        ("page_range", "first3"),
        ("output_formats", "mono,dual,side_by_side"),
    ] {
        body.extend(
            format!(
                "--UIBoundary\r\nContent-Disposition: form-data; name=\"{key}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    if let Some(pdf) = pdf {
        body.extend(b"--UIBoundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"paper.pdf\"\r\nContent-Type: application/pdf\r\n\r\n");
        body.extend(pdf);
        body.extend(b"\r\n");
    }
    body.extend(b"--UIBoundary--\r\n");
    body
}
async fn wait(c: &reqwest::Client, base: &str, id: &str) -> Result<Value> {
    for _ in 0..600 {
        let job: Value = c
            .get(format!("{base}/api/ui/jobs/{id}"))
            .send()
            .await?
            .json()
            .await?;
        if ["done", "error", "cancelled"].contains(&job["status"].as_str().unwrap_or("")) {
            return Ok(job);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    anyhow::bail!("job timed out")
}
#[tokio::test(flavor = "current_thread")]
async fn translator_contract_persists_secrets_privately_and_exports_once() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let web = tmp.path().join("web");
    std::fs::create_dir(&web)?;
    std::fs::write(web.join("index.html"), "translator fixture")?;
    std::fs::write(web.join("chunk.js"), "console.log(1)")?;
    let data = tmp.path().join("data");
    let fixture = tmp.path().join("fixture.pdf");
    pdf::fixture(&fixture)?;
    let bytes = std::fs::read(fixture)?;
    let requests = Arc::new(AtomicUsize::new(0));
    let calls = requests.clone();
    let mock = axum::Router::new().route(
        "/chat/completions",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, Ordering::Relaxed);
                let source = body["messages"][1]["content"].as_str().unwrap_or("");
                let markers = regex::Regex::new("⟦[0-9]+⟧")
                    .unwrap()
                    .find_iter(source)
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                axum::Json(
                    json!({"choices":[{"message":{"content":format!("‣ 中文译文。{markers}")}}]}),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mock_url = format!("http://{}", listener.local_addr()?);
    let mock_task = tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });
    let (server, base) = start(&data, &web).await?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    assert_eq!(
        c.get(&base).send().await?.text().await?,
        "translator fixture"
    );
    assert!(
        c.get(format!("{base}/chunk.js")).send().await?.headers()["content-type"]
            .to_str()?
            .starts_with("application/javascript")
    );
    assert_eq!(
        c.get(format!("{base}/api/not-a-route"))
            .send()
            .await?
            .status(),
        404
    );
    let secret = "test-secret-private-settings-only";
    let patch = json!({"services":[{"id":"openai","base_url":mock_url,"model":"ui-contract","api_key":secret}],"defaults":{"service":"openai","output_formats":["mono","dual","side_by_side"]}});
    let response = c
        .put(format!("{base}/api/ui/settings"))
        .json(&patch)
        .send()
        .await?;
    assert!(response.status().is_success());
    let public = response.text().await?;
    assert!(!public.contains(secret));
    assert!(public.contains("\"has_key\":true"));
    assert!(std::fs::read_to_string(data.join("ui-settings.json"))?.contains(secret));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(data.join("ui-settings.json"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let term: Value = c
        .post(format!("{base}/api/ui/glossary/terms"))
        .json(&json!({"term":"neural","zh":"神经"}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(term["terms"]["neural"], "神经");
    let job: Value = c
        .post(format!("{base}/api/ui/jobs"))
        .header("content-type", "multipart/form-data; boundary=UIBoundary")
        .body(multipart(Some(&bytes)))
        .send()
        .await?
        .json()
        .await?;
    let id = job["id"].as_str().expect("job id");
    let first = wait(&c, &base, id).await?;
    assert_eq!(first["status"], "done", "{first}");
    assert_eq!(first["pages"], 1);
    assert_eq!(first["options"]["service"], "openai");
    assert_eq!(first["options"]["page_range"], "first3");
    let count = requests.load(Ordering::Relaxed);
    assert!(count > 0);
    let mut sequences = Vec::new();
    for _ in 0..2 {
        let events = c
            .get(format!("{base}/api/ui/jobs/{id}/events"))
            .send()
            .await?
            .text()
            .await?;
        assert!(!events.contains("event: job"));
        let payload = events
            .lines()
            .find_map(|l| l.strip_prefix("data: "))
            .unwrap();
        let value: Value = serde_json::from_str(payload)?;
        assert_eq!(value["type"], "done");
        sequences.push(value["seq"].as_u64().unwrap());
    }
    assert!(sequences[1] > sequences[0]);
    for (kind, pages) in [("mono", 1), ("dual", 2), ("side_by_side", 1)] {
        let response = c
            .get(format!("{base}{}", first["files"][kind].as_str().unwrap()))
            .send()
            .await?;
        let filename_kind = match kind {
            "side_by_side" => "dual",
            "dual" => "interleaved",
            _ => "mono",
        };
        assert!(
            response.headers()["content-disposition"]
                .to_str()?
                .ends_with(&format!("-{filename_kind}.pdf\""))
        );
        assert!(
            response.headers()["content-disposition"]
                .to_str()?
                .starts_with("attachment")
        );
        let pdf_path = tmp.path().join(format!("{kind}.pdf"));
        std::fs::write(&pdf_path, response.bytes().await?)?;
        let doc = pdf::open(&pdf_path, None)?;
        assert_eq!(doc.page_count()?, pages);
    }
    let content: Value = c
        .get(format!("{base}/api/ui/papers/{id}/content"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(content["has_content"], false);
    let settings = std::fs::read_to_string(data.join(id).join("settings.json"))?;
    assert!(!settings.contains(secret));
    assert!(!data.join(id).join("job.json").exists());
    // Retained-input retry must reuse translations even when three exports are selected.
    let retry: Value = c
        .post(format!("{base}/api/ui/jobs/{id}/retry"))
        .header("content-type", "multipart/form-data; boundary=UIBoundary")
        .body(multipart(None))
        .send()
        .await?
        .json()
        .await?;
    let retry_id = retry["id"].as_str().unwrap();
    let retried = wait(&c, &base, retry_id).await?;
    assert_eq!(retried["status"], "done", "{retried}");
    assert_eq!(requests.load(Ordering::Relaxed), count);
    let deleted: Value = c
        .delete(format!("{base}/api/ui/papers/{id}"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(deleted["files_retained"], true);
    assert!(data.join(id).join("input.pdf").exists());
    drop(server);
    let (_server, base) = start(&data, &web).await?;
    let response = c
        .get(format!("{base}/api/ui/settings"))
        .send()
        .await?
        .text()
        .await?;
    assert!(response.contains("\"has_key\":true"));
    assert!(!response.contains(secret));
    let listed: Vec<Value> = c
        .get(format!("{base}/api/ui/papers"))
        .send()
        .await?
        .json()
        .await?;
    assert!(!listed.iter().any(|v| v["id"] == id));
    assert!(listed.iter().any(|v| v["id"] == retry_id));
    mock_task.abort();
    Ok(())
}
