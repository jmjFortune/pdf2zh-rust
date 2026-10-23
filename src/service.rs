mod ui;
use crate::engine::{self, JobConfig, OutputFormat, Progress};
use crate::translate::TranslationSettings;
use anyhow::{Context, Result};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures_util::stream;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

struct Store {
    db: Mutex<Connection>,
    root: PathBuf,
    previews: tokio::sync::Semaphore,
    preferences: Mutex<ui::Preferences>,
    web_root: PathBuf,
}
type Shared = Arc<Store>;
type WebResult<T> = std::result::Result<T, WebError>;
fn require(condition: bool, message: &str) -> WebResult<()> {
    if condition {
        Ok(())
    } else {
        Err(WebError(anyhow::anyhow!("{message}")))
    }
}
struct WebError(anyhow::Error);
struct UploadCleanup {
    path: PathBuf,
    keep: bool,
}
impl Drop for UploadCleanup {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
impl<E: Into<anyhow::Error>> From<E> for WebError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":self.0.to_string(),"detail":self.0.to_string()})),
        )
            .into_response()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: String,
    pub filename: String,
    pub status: String,
    pub created: i64,
    pub progress: Option<Progress>,
    pub error: Option<String>,
    #[serde(default)]
    pub output_pages: Option<usize>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct UploadOptions {
    translation: TranslationSettings,
    pages: String,
    format: Option<OutputFormat>,
    additional_formats: Vec<OutputFormat>,
    layout: Option<bool>,
    password: Option<String>,
}

impl Store {
    fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobInfo> {
        let progress: Option<String> = row.get(4)?;
        Ok(JobInfo {
            id: row.get(0)?,
            filename: row.get(1)?,
            status: row.get(2)?,
            created: row.get(3)?,
            progress: progress.and_then(|p| serde_json::from_str(&p).ok()),
            error: row.get(5)?,
            output_pages: None,
        })
    }
    fn get(&self, id: &str) -> Result<JobInfo> {
        uuid::Uuid::parse_str(id).context("invalid task id")?;
        let mut job = self.db.lock().unwrap().query_row(
            "SELECT id,filename,status,created,progress,error FROM jobs WHERE id=?1",
            [id],
            Self::row,
        )?;
        self.metadata(&mut job);
        Ok(job)
    }
    fn list(&self) -> Result<Vec<JobInfo>> {
        let db = self.db.lock().unwrap();
        let mut stmt=db.prepare("SELECT id,filename,status,created,progress,error FROM jobs ORDER BY created DESC, rowid DESC LIMIT 100")?;
        let mut jobs = stmt
            .query_map([], Self::row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for job in &mut jobs {
            self.metadata(job);
        }
        Ok(jobs)
    }
    fn metadata(&self, job: &mut JobInfo) {
        if let Ok(bytes) = std::fs::read(self.root.join(&job.id).join("completion.json"))
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            job.output_pages = value["output_pages"].as_u64().map(|v| v as usize);
        }
    }
    fn status(&self, id: &str, status: &str, error: Option<&str>) -> Result<()> {
        self.db.lock().unwrap().execute(
            "UPDATE jobs SET status=?2,error=?3 WHERE id=?1",
            params![id, status, error],
        )?;
        Ok(())
    }
}

pub async fn serve(root: PathBuf, listen: String) -> Result<()> {
    std::fs::create_dir_all(&root)?;
    let root = std::fs::canonicalize(root)?;
    let db = Connection::open(root.join("jobs.sqlite3"))?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,filename TEXT NOT NULL,status TEXT NOT NULL,created INTEGER NOT NULL DEFAULT(unixepoch()),progress TEXT,error TEXT); UPDATE jobs SET status='interrupted',error='Service restarted during translation' WHERE status IN ('running','cancelling');")?;
    let store = Arc::new(Store {
        db: Mutex::new(db),
        preferences: Mutex::new(ui::Preferences::load(&root)?),
        web_root: std::env::var_os("PDF2ZH_WEB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let adjacent = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.join("web")));
                adjacent
                    .filter(|p| p.join("index.html").is_file())
                    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/out"))
            }),
        root,
        previews: tokio::sync::Semaphore::new(1),
    });
    let scheduler = tokio::spawn(schedule(store.clone()));
    let app=Router::new().nest("/api/ui", ui::routes()).fallback(ui::static_file)
        .route("/api/health",get(||async {Json(serde_json::json!({"status":"ok","engine":"rust","version":env!("CARGO_PKG_VERSION")}))}))
        .route("/api/jobs",get(list).post(upload)).route("/api/jobs/{id}",get(detail))
        .route("/api/jobs/{id}/cancel",post(cancel)).route("/api/jobs/{id}/events",get(events))
        .route("/api/jobs/{id}/retry",post(retry))
        .route("/api/jobs/{id}/output",get(output)).route("/api/jobs/{id}/input",get(input))
        .route("/api/jobs/{id}/preview/{page}",get(preview))
        .layer(DefaultBodyLimit::max(200*1024*1024)).with_state(store);
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    eprintln!("pdf2zh-rust listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    scheduler.abort();
    let _ = scheduler.await;
    Ok(())
}

async fn list(State(s): State<Shared>) -> WebResult<Json<Vec<JobInfo>>> {
    Ok(Json(s.list()?))
}
async fn detail(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Json<JobInfo>> {
    Ok(Json(s.get(&id)?))
}

async fn upload(State(s): State<Shared>, mut form: Multipart) -> WebResult<Json<JobInfo>> {
    let id = uuid::Uuid::new_v4().to_string();
    let directory = s.root.join(&id);
    tokio::fs::create_dir(&directory).await?;
    let mut cleanup = UploadCleanup {
        path: directory.clone(),
        keep: false,
    };
    let mut filename = None;
    let mut options = UploadOptions::default();
    while let Some(mut field) = form.next_field().await? {
        match field.name() {
            Some("file") => {
                require(filename.is_none(), "upload one PDF per task")?;
                filename = Some(
                    field
                        .file_name()
                        .unwrap_or("document.pdf")
                        .chars()
                        .take(180)
                        .collect::<String>(),
                );
                let mut file = tokio::fs::File::create(directory.join("input.pdf")).await?;
                let mut total = 0usize;
                while let Some(chunk) = field.chunk().await? {
                    total += chunk.len();
                    require(
                        total <= 190 * 1024 * 1024,
                        "PDF exceeds 190 MiB upload limit",
                    )?;
                    file.write_all(&chunk).await?;
                }
                require(total > 0, "PDF is empty")?;
            }
            Some("options") => {
                let text = field.text().await?;
                require(text.len() < 128 * 1024, "options too large")?;
                options = serde_json::from_str(&text)?;
            }
            _ => {}
        }
    }
    let filename = filename.context("missing PDF upload")?;
    enqueue(&s, &id, &filename, options)?;
    cleanup.keep = true;
    Ok(Json(s.get(&id)?))
}

// New settings, including credentials, must be supplied explicitly. Reuse only
// the retained PDF; leave the failed record and its progress intact.
async fn retry(
    State(s): State<Shared>,
    Path(source): Path<String>,
    Json(options): Json<UploadOptions>,
) -> WebResult<Json<JobInfo>> {
    let old = s.get(&source)?;
    require(
        ["failed", "interrupted", "cancelled"].contains(&old.status.as_str()),
        "only stopped tasks can be retried",
    )?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = s.root.join(&id);
    tokio::fs::create_dir(&directory).await?;
    let mut cleanup = UploadCleanup {
        path: directory.clone(),
        keep: false,
    };
    let input = s.root.join(source).join("input.pdf");
    if tokio::fs::hard_link(&input, directory.join("input.pdf"))
        .await
        .is_err()
    {
        tokio::fs::copy(&input, directory.join("input.pdf")).await?;
    }
    enqueue(&s, &id, &old.filename, options)?;
    cleanup.keep = true;
    Ok(Json(s.get(&id)?))
}

fn enqueue(s: &Store, id: &str, filename: &str, options: UploadOptions) -> Result<()> {
    let directory = s.root.join(id);
    let config = JobConfig {
        input: directory.join("input.pdf"),
        output: directory.join("output.pdf"),
        data_dir: directory.clone(),
        cache_path: Some(s.root.join("cache.sqlite3")),
        translation: options.translation,
        pages: options.pages,
        format: options.format.unwrap_or(OutputFormat::Mono),
        additional_formats: options.additional_formats,
        layout: options.layout.unwrap_or(true),
        password: options.password,
        ..Default::default()
    };
    ui::save_snapshot(&directory, &config)?;
    let path = directory.join("job.json");
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(&path)?
        .write_all(&serde_json::to_vec(&config)?)?;
    s.db.lock().unwrap().execute(
        "INSERT INTO jobs(id,filename,status) VALUES(?1,?2,'queued')",
        params![id, filename],
    )?;
    Ok(())
}

async fn cancel(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Json<JobInfo>> {
    s.get(&id)?;
    let queued = s.db.lock().unwrap().execute(
        "UPDATE jobs SET status='cancelled' WHERE id=?1 AND status='queued'",
        [&id],
    )?;
    if queued > 0 {
        let _ = tokio::fs::remove_file(s.root.join(&id).join("job.json")).await;
    } else if s.db.lock().unwrap().execute(
        "UPDATE jobs SET status='cancelling' WHERE id=?1 AND status='running'",
        [&id],
    )? > 0
    {
        tokio::fs::write(s.root.join(&id).join("cancel"), b"cancel").await?;
    }
    Ok(Json(s.get(&id)?))
}

async fn output(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Response> {
    require(
        s.get(&id)?.status == "completed",
        "translation result is not available",
    )?;
    file_response(s.root.join(id).join("output.pdf"), "application/pdf").await
}
async fn input(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Response> {
    s.get(&id)?;
    file_response(s.root.join(id).join("input.pdf"), "application/pdf").await
}
async fn preview(
    State(s): State<Shared>,
    Path((id, page)): Path<(String, usize)>,
) -> WebResult<Response> {
    let job = s.get(&id)?;
    require(
        job.status == "completed",
        "translation result is not available",
    )?;
    require(
        page >= 1 && page <= job.output_pages.unwrap_or(1),
        "preview page is outside document",
    )?;
    let directory = s.root.join(&id);
    let path = directory.join(format!("preview-{page}.png"));
    let _permit = s.previews.acquire().await?;
    if !path.is_file() {
        let temp = directory.join(format!("preview-{page}.partial.png"));
        let mut child = Command::new(std::env::current_exe()?);
        child
            .arg("render")
            .arg(directory.join("output.pdf"))
            .arg(&temp)
            .arg("--page")
            .arg(page.to_string())
            .arg("--dpi")
            .arg("96")
            .kill_on_drop(true)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let result = tokio::time::timeout(Duration::from_secs(30), child.status()).await??;
        require(
            result.success(),
            "preview rendering failed; download the PDF instead",
        )?;
        tokio::fs::rename(temp, &path).await?;
    }
    file_response(path, "image/png").await
}
async fn file_response(path: PathBuf, mime: &'static str) -> WebResult<Response> {
    // Stream files. Reading an entire large PDF into the resident process would
    // defeat its bounded-memory design.
    let file = tokio::fs::File::open(path).await?;
    let stream = stream::try_unfold(file, |mut file| async move {
        let mut buf = vec![0u8; 64 * 1024];
        let n = file.read(&mut buf).await?;
        if n == 0 {
            Ok::<_, std::io::Error>(None)
        } else {
            buf.truncate(n);
            Ok(Some((buf, file)))
        }
    });
    Ok(([(header::CONTENT_TYPE, mime)], Body::from_stream(stream)).into_response())
}

async fn events(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> WebResult<Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>>> {
    s.get(&id)?;
    let events = stream::unfold((s, id, false), |(s, id, done)| async move {
        if done {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let job = s.get(&id).ok()?;
        let done = terminal(&job.status);
        let event = Event::default().event("job").json_data(job).ok()?;
        Some((Ok(event), (s, id, done)))
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

fn terminal(status: &str) -> bool {
    ["completed", "failed", "cancelled", "interrupted"].contains(&status)
}

async fn schedule(store: Shared) {
    loop {
        let next = {
            let db = store.db.lock().unwrap();
            db.query_row(
                "SELECT id FROM jobs WHERE status='queued' ORDER BY created,rowid LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
        };
        if let Some(id) = next {
            if let Err(e) = execute(store.clone(), &id).await {
                let _ = store.status(&id, "failed", Some(&e.to_string()));
            }
        } else {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }
}

async fn execute(store: Shared, id: &str) -> Result<()> {
    // Only the scheduler starts workers. Cancellation can remove queued work
    // before it is claimed, so update conditionally.
    let changed = store.db.lock().unwrap().execute(
        "UPDATE jobs SET status='running' WHERE id=?1 AND status='queued'",
        [id],
    )?;
    if changed == 0 {
        return Ok(());
    }
    let directory = store.root.join(id);
    let mut child = Command::new(std::env::current_exe()?)
        .arg("worker")
        .arg("--job")
        .arg(directory.join("job.json"))
        .env("PDF2ZH_PARENT_PID", std::process::id().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child.stdout.take().context("missing worker stdout")?;
    let stderr = child.stderr.take().context("missing worker stderr")?;
    let s = store.clone();
    let job_id = id.to_string();
    let reader = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            let n = reader.read_until(b'\n', &mut bytes).await?;
            if n == 0 {
                break;
            }
            anyhow::ensure!(n < 64 * 1024, "worker event exceeded size limit");
            if let Ok(event) = serde_json::from_slice::<Progress>(&bytes) {
                s.db.lock().unwrap().execute(
                    "UPDATE jobs SET progress=?2 WHERE id=?1",
                    params![job_id, serde_json::to_string(&event)?],
                )?;
            }
        }
        Ok::<_, anyhow::Error>(())
    });
    let errors = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = [0u8; 4096];
        let mut tail = Vec::new();
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            tail.extend_from_slice(&buf[..n]);
            if tail.len() > 8192 {
                tail.drain(..tail.len() - 8192);
            }
        }
        Ok::<_, std::io::Error>(String::from_utf8_lossy(&tail).to_string())
    });
    let mut cancelling = None;
    let status = loop {
        if let Some(exit) = child.try_wait()? {
            break exit;
        }
        if directory.join("cancel").exists() {
            let start = cancelling.get_or_insert_with(Instant::now);
            if start.elapsed() > Duration::from_secs(3) {
                child.kill().await?;
                break child.wait().await?;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    reader.await??;
    let error = errors.await??;
    if directory.join("cancel").exists() {
        for name in [
            "output.pdf",
            "output-mono.pdf",
            "output-dual.pdf",
            "output-side_by_side.pdf",
            "completion.json",
            "preview-1.png",
        ] {
            let _ = tokio::fs::remove_file(directory.join(name)).await;
        }
        store.status(id, "cancelled", None)?;
    } else if status.success() && directory.join("output.pdf").is_file() {
        store.status(id, "completed", None)?;
    } else {
        store.status(
            id,
            "failed",
            Some(if error.trim().is_empty() {
                "Translation worker failed"
            } else {
                error.trim()
            }),
        )?;
    }
    // Credentials are only needed for the task. Keep input/output and public
    // progress in history; remove its private settings snapshot after completion.
    let _ = tokio::fs::remove_file(directory.join("job.json")).await;
    if let Ok(mut entries) = tokio::fs::read_dir(&directory).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_name().to_string_lossy().starts_with("analysis-") {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
    Ok(())
}

pub async fn worker(job: PathBuf) -> Result<()> {
    crate::analysis::guard_parent()?;
    let config: JobConfig = serde_json::from_slice(&std::fs::read(job)?)?;
    #[cfg(unix)]
    if let Ok(parent) = std::env::var("PDF2ZH_PARENT_PID")
        && let Ok(parent) = parent.parse::<u32>()
    {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if unsafe { libc::getppid() } as u32 != parent {
                    std::process::exit(130);
                }
            }
        });
    }
    engine::run(&config, |event| {
        println!("{}", serde_json::to_string(&event).unwrap());
    })
    .await?;
    let output = crate::pdf::open(&config.output, None)?;
    let pages = output.page_count()?;
    drop(output);
    std::fs::write(
        config.data_dir.join("completion.json"),
        serde_json::to_vec(&serde_json::json!({"output_pages":pages}))?,
    )?;
    if let Err(error) = crate::pdf::render(
        &config.output,
        &config.data_dir.join("preview-1.png"),
        0,
        96.,
    ) {
        eprintln!("Preview unavailable: {error}");
    }
    Ok(())
}
