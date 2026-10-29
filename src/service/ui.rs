//! Adapter for the retained translator frontend. Legacy /api/jobs remains stable.
use super::*;
use axum::extract::{OriginalUri, Query};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path as FsPath};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Preferences {
    services: BTreeMap<String, Value>,
    defaults: Value,
    terms: BTreeMap<String, String>,
    hidden: BTreeSet<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            services: BTreeMap::new(),
            defaults: json!({"lang_in":"en","lang_out":"zh-CN","service":"google","thread":1,"output_formats":["mono"]}),
            terms: BTreeMap::new(),
            hidden: BTreeSet::new(),
        }
    }
}
impl Preferences {
    pub(super) fn load(root: &FsPath) -> Result<Self> {
        let path = root.join("ui-settings.json");
        if path.exists() {
            Ok(serde_json::from_slice(&std::fs::read(path)?)?)
        } else {
            Ok(Self::default())
        }
    }
    fn save(&self, root: &FsPath) -> Result<()> {
        private_json(&root.join("ui-settings.json"), self)
    }
}
fn private_json(path: &FsPath, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = opts.open(&tmp)?;
        file.write_all(&serde_json::to_vec(value)?)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}
fn profiles() -> Value {
    serde_json::from_str(include_str!("profiles.json"))
        .expect("bundled provider catalogue must be valid JSON")
}
fn profile(id: &str) -> Result<Value> {
    profiles()
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == id)
        .cloned()
        .context("unsupported translation service")
}
fn settings_value(p: &Preferences) -> Value {
    let services: Vec<Value> = profiles()
        .as_array()
        .unwrap()
        .iter()
        .map(|base| {
            let mut value = base.clone();
            let id = base["id"].as_str().unwrap();
            let saved = p.services.get(id);
            for name in ["base_url", "model"] {
                value[name] = saved
                    .and_then(|s| s.get(name))
                    .cloned()
                    .unwrap_or_else(|| base[format!("default_{name}")].clone());
            }
            let mut options = json!({});
            for field in base["option_fields"].as_array().unwrap() {
                let key = field["key"].as_str().unwrap();
                options[key] = saved
                    .and_then(|v| v.get("options"))
                    .and_then(|v| v.get(key))
                    .cloned()
                    .unwrap_or_else(|| field["default"].clone());
            }
            value["options"] = options;
            value["has_key"] = json!(
                saved
                    .and_then(|s| s["api_key"].as_str())
                    .is_some_and(|s| !s.is_empty())
            );
            value
        })
        .collect();
    json!({"services":services,"defaults":p.defaults,"glossary_count":p.terms.len()})
}
fn valid_url(value: &str) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    let url = reqwest::Url::parse(value).context("invalid API base URL")?;
    anyhow::ensure!(
        ["http", "https"].contains(&url.scheme())
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "API URL must use HTTP(S) without embedded credentials, query or fragment"
    );
    Ok(())
}
fn formats(value: &str) -> Result<Vec<OutputFormat>> {
    let mut result = Vec::new();
    for name in value.split(',') {
        let format = match name.trim() {
            "mono" => OutputFormat::Mono,
            "dual" | "interleaved" => OutputFormat::Interleaved,
            "side_by_side" => OutputFormat::SideBySide,
            _ => anyhow::bail!("unsupported output format"),
        };
        if !result.contains(&format) {
            result.push(format);
        }
    }
    anyhow::ensure!(!result.is_empty(), "select an output format");
    Ok(result)
}
pub(super) fn routes() -> Router<Shared> {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ok","engine":"rust","mock":false})) }),
        )
        .route("/settings", get(settings).put(save_settings))
        .route("/glossary", get(glossary))
        .route("/glossary/terms", post(add_term).delete(delete_term))
        .route("/jobs", get(jobs).post(create_job))
        .route("/jobs/active", get(active_job))
        .route("/jobs/{id}", get(job))
        .route("/jobs/{id}/cancel", post(cancel_job))
        .route("/jobs/{id}/retry", post(retry_job))
        .route("/jobs/{id}/events", get(job_events))
        .route("/jobs/{id}/download/{kind}", get(download))
        .route("/papers", get(papers))
        .route("/papers/{id}", get(paper).delete(hide_paper))
        .route("/papers/{id}/content", get(content))
}
async fn settings(State(s): State<Shared>) -> Json<Value> {
    Json(settings_value(&s.preferences.lock().unwrap()))
}
async fn save_settings(
    State(s): State<Shared>,
    Json(patch): Json<Value>,
) -> WebResult<Json<Value>> {
    let mut guard = s.preferences.lock().unwrap();
    let mut next = guard.clone();
    if let Some(services) = patch["services"].as_array() {
        for item in services {
            let id = item["id"].as_str().context("service id required")?;
            let spec = profile(id)?;
            let saved = next.services.entry(id.into()).or_insert(json!({}));
            for field in ["base_url", "model", "api_key"] {
                if let Some(value) = item.get(field) {
                    let value = value.as_str().context("settings must be strings")?;
                    require(value.len() <= 8192, "setting too long")?;
                    if field == "base_url" {
                        valid_url(value)?;
                    }
                    saved[field] = json!(value.trim());
                }
            }
            if let Some(options) = item.get("options") {
                let options = options
                    .as_object()
                    .context("provider options must be an object")?;
                if !saved["options"].is_object() {
                    saved["options"] = json!({});
                }
                for (key, value) in options {
                    require(
                        spec["option_fields"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|field| field["key"] == *key),
                        "unknown provider option",
                    )?;
                    let value = value.as_str().context("provider option must be text")?;
                    require(
                        value.len() <= 128 && !value.chars().any(char::is_control),
                        "invalid provider option",
                    )?;
                    saved["options"][key] = json!(value.trim());
                }
            }
        }
    }
    if let Some(defaults) = patch.get("defaults") {
        for field in ["lang_in", "lang_out", "service", "output_formats"] {
            if let Some(value) = defaults.get(field) {
                next.defaults[field] = value.clone();
            }
        }
        profile(
            next.defaults["service"]
                .as_str()
                .context("default service required")?,
        )?;
        for field in ["lang_in", "lang_out"] {
            require(
                next.defaults[field]
                    .as_str()
                    .is_some_and(|v| !v.is_empty() && v.len() < 32),
                "invalid language",
            )?;
        }
        let list = next.defaults["output_formats"]
            .as_array()
            .context("output formats must be a list")?;
        formats(
            &list
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
        )?;
        next.defaults["thread"] = json!(1); // Native worker translates serially.
    }
    next.save(&s.root)?;
    *guard = next;
    Ok(Json(settings_value(&guard)))
}
fn glossary_value(p: &Preferences) -> Value {
    json!({"terms":p.terms,"presets":[]})
}
async fn glossary(State(s): State<Shared>) -> Json<Value> {
    Json(glossary_value(&s.preferences.lock().unwrap()))
}
async fn add_term(State(s): State<Shared>, Json(value): Json<Value>) -> WebResult<Json<Value>> {
    let term = value["term"].as_str().unwrap_or("").trim();
    let zh = value["zh"].as_str().unwrap_or("").trim();
    require(
        !term.is_empty() && !zh.is_empty() && term.len() < 1024 && zh.len() < 4096,
        "term and translation required (maximum 1024/4096 bytes)",
    )?;
    let mut p = s.preferences.lock().unwrap();
    let mut next = p.clone();
    require(
        next.terms.len() < 5000 || next.terms.contains_key(term),
        "glossary limit reached",
    )?;
    next.terms.insert(term.into(), zh.into());
    next.save(&s.root)?;
    *p = next;
    Ok(Json(glossary_value(&p)))
}
async fn delete_term(
    State(s): State<Shared>,
    Query(q): Query<BTreeMap<String, String>>,
) -> WebResult<Json<Value>> {
    let mut p = s.preferences.lock().unwrap();
    let mut next = p.clone();
    if let Some(term) = q.get("term") {
        next.terms.remove(term);
    }
    next.save(&s.root)?;
    *p = next;
    Ok(Json(glossary_value(&p)))
}

fn public_url(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    match reqwest::Url::parse(value) {
        Ok(mut url) => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string().trim_end_matches('/').to_owned()
        }
        Err(_) => String::new(),
    }
}

pub(super) fn save_snapshot(dir: &FsPath, config: &JobConfig) -> Result<()> {
    // Deliberately allowlist public metadata: never serialize credentials/password/options.
    let mut exports = vec![config.format];
    for f in &config.additional_formats {
        if !exports.contains(f) {
            exports.push(*f);
        }
    }
    private_json(
        &dir.join("settings.json"),
        &json!({"service":config.translation.provider,"lang_in":config.translation.source,"lang_out":config.translation.target,"base_url":public_url(&config.translation.base_url),"model":config.translation.model,"page_range":config.pages,"output_formats":exports.iter().map(|f|f.ui_name()).collect::<Vec<_>>(),"thread":1,"provider_options":config.translation.options.iter().filter(|(k,_)|["region","api_version"].contains(&k.as_str())).collect::<BTreeMap<_,_>>()}),
    )
}
fn snapshot(s: &Store, id: &str) -> Value {
    std::fs::read(s.root.join(id).join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}
fn job_value(s: &Store, job: &JobInfo) -> Value {
    let options = snapshot(s, &job.id);
    let pages = job.progress.as_ref().map_or(0, |p| p.total);
    let status = match job.status.as_str() {
        "completed" => "done",
        "failed" | "interrupted" => "error",
        "cancelling" => "running",
        s => s,
    };
    let mut files = json!({});
    if status == "done" {
        let list = options["output_formats"].as_array();
        if let Some(list) = list {
            for (i, name) in list.iter().filter_map(Value::as_str).enumerate() {
                let path = if i == 0 {
                    s.root.join(&job.id).join("output.pdf")
                } else {
                    s.root.join(&job.id).join(format!("output-{name}.pdf"))
                };
                if path.is_file() {
                    files[name] = json!(format!("/api/ui/jobs/{}/download/{}", job.id, name));
                }
            }
        } else {
            let name = if job.output_pages == Some(pages * 2) && pages > 0 {
                "dual"
            } else {
                "mono"
            };
            files[name] = json!(format!("/api/ui/jobs/{}/download/{}", job.id, name));
        }
    }
    let stage = if status == "done" {
        "done"
    } else {
        job.progress.as_ref().map_or("queued", |p| p.stage.as_str())
    };
    let message = if let Some(p) = &job.progress {
        format!(
            "{} · page {}/{} · {} translated blocks{}{}",
            stage,
            p.page,
            p.total,
            p.completed_blocks,
            if job.status == "cancelling" {
                " · cancelling…"
            } else {
                ""
            },
            if p.message.is_empty() {
                String::new()
            } else {
                format!(" · {}", p.message)
            }
        )
    } else {
        job.status.clone()
    };
    let current = if status == "done" {
        pages
    } else {
        job.progress
            .as_ref()
            .map_or(0, |p| p.page.saturating_sub(1))
    };
    json!({"id":job.id,"filename":job.filename,"status":status,"created_at":iso_date(job.created),"size":std::fs::metadata(s.root.join(&job.id).join("input.pdf")).map_or(0,|m|m.len()),"pages":pages,"progress":{"current":current,"total":pages},"stage":stage,"message":message,"error":job.error,"files":files,"options":options})
}
fn iso_date(seconds: i64) -> String {
    // SQLite's UTC formatter avoids adding a date library solely for metadata.
    Connection::open_in_memory()
        .and_then(|db| {
            db.query_row(
                "SELECT strftime('%Y-%m-%dT%H:%M:%SZ',?1,'unixepoch')",
                [seconds],
                |r| r.get(0),
            )
        })
        .unwrap_or_default()
}
async fn job(State(s): State<Shared>, Path(id): Path<String>) -> Response {
    match s.get(&id) {
        Ok(j) => Json(job_value(&s, &j)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail":"Task not found"})),
        )
            .into_response(),
    }
}
async fn jobs(State(s): State<Shared>) -> WebResult<Json<Vec<Value>>> {
    Ok(Json(s.list()?.iter().map(|j| job_value(&s, j)).collect()))
}
async fn active_job(State(s): State<Shared>) -> WebResult<Json<Value>> {
    Ok(Json(
        s.list()?
            .iter()
            .find(|j| !terminal(&j.status))
            .map(|j| job_value(&s, j))
            .unwrap_or(Value::Null),
    ))
}
async fn cancel_job(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let _ = super::cancel(State(s.clone()), Path(id.clone())).await?;
    Ok(Json(job_value(&s, &s.get(&id)?)))
}

// Resolve a UI profile to the actual protocol before queuing a model/PDF worker.
fn translation_settings(
    prefs: &Preferences,
    service: &str,
    source: String,
    target: String,
) -> Result<TranslationSettings> {
    let base = profile(service)?;
    let saved = prefs.services.get(service);
    let setting = |name: &str| {
        saved
            .and_then(|v| v.get(name))
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| base[format!("default_{name}")].as_str().unwrap_or(""))
            .to_owned()
    };
    let key = saved
        .and_then(|v| v["api_key"].as_str())
        .filter(|v| !v.is_empty())
        .map(str::to_owned);
    anyhow::ensure!(
        !base["needs_key"].as_bool().unwrap_or(false) || key.is_some(),
        "Configure {} API key in Settings first",
        base["name"].as_str().unwrap()
    );
    let provider = base["provider"]
        .as_str()
        .context("profile lacks protocol")?
        .to_owned();
    let base_url = setting("base_url");
    let model = setting("model");
    anyhow::ensure!(
        !base["requires_model"].as_bool().unwrap_or(false) || !model.is_empty(),
        "Set a model or deployment name for {} in Settings",
        base["name"].as_str().unwrap()
    );
    anyhow::ensure!(
        ![
            "openai",
            "azureopenai",
            "azure",
            "dify",
            "anythingllm",
            "deeplx"
        ]
        .contains(&provider.as_str())
            || !base_url.is_empty(),
        "Set the API URL for {} in Settings",
        base["name"].as_str().unwrap()
    );
    valid_url(&base_url)?;
    let mut options = BTreeMap::new();
    for field in base["option_fields"].as_array().unwrap() {
        let name = field["key"].as_str().unwrap();
        let value = saved
            .and_then(|v| v.get("options"))
            .and_then(|v| v.get(name))
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| field["default"].as_str().unwrap_or(""));
        options.insert(name.to_owned(), value.to_owned());
    }
    Ok(TranslationSettings {
        provider,
        source,
        target,
        base_url,
        model,
        api_key: if base["api_key_optional"].as_bool().unwrap_or(false) {
            Some(key.unwrap_or_default())
        } else {
            key
        },
        options,
        glossary: prefs.terms.clone(),
        ..Default::default()
    })
}

async fn create_job(State(s): State<Shared>, form: Multipart) -> WebResult<Json<Value>> {
    receive(s, form, None).await
}
async fn retry_job(
    State(s): State<Shared>,
    Path(id): Path<String>,
    form: Multipart,
) -> WebResult<Json<Value>> {
    receive(s, form, Some(id)).await
}
async fn receive(s: Shared, mut form: Multipart, source: Option<String>) -> WebResult<Json<Value>> {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = s.root.join(&id);
    tokio::fs::create_dir(&dir).await?;
    let mut cleanup = UploadCleanup {
        path: dir.clone(),
        keep: false,
    };
    let mut fields = BTreeMap::new();
    let mut filename = None;
    if let Some(source) = source {
        let old = s.get(&source)?;
        require(terminal(&old.status), "task is still running")?;
        tokio::fs::copy(s.root.join(source).join("input.pdf"), dir.join("input.pdf")).await?;
        filename = Some(old.filename);
    }
    while let Some(mut field) = form.next_field().await? {
        let name = field.name().unwrap_or("").to_owned();
        if name == "file" {
            require(filename.is_none(), "upload one PDF per task")?;
            let name = field
                .file_name()
                .unwrap_or("document.pdf")
                .chars()
                .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
                .take(180)
                .collect::<String>();
            require(name.to_lowercase().ends_with(".pdf"), "choose a PDF file")?;
            filename = Some(name);
            let mut file = tokio::fs::File::create(dir.join("input.pdf")).await?;
            let mut total = 0;
            while let Some(chunk) = field.chunk().await? {
                total += chunk.len();
                require(
                    total <= 190 * 1024 * 1024,
                    "PDF exceeds 190 MiB upload limit",
                )?;
                file.write_all(&chunk).await?;
            }
            require(total > 0, "PDF is empty")?;
        } else {
            let text = field.text().await?;
            require(text.len() < 8192, "field too large")?;
            fields.insert(name, text);
        }
    }
    let filename = filename.context("missing PDF upload")?;
    let prefs = s.preferences.lock().unwrap().clone();
    let service = fields
        .get("service")
        .map(String::as_str)
        .unwrap_or("google");
    let output_formats = formats(
        fields
            .get("output_formats")
            .map(String::as_str)
            .unwrap_or("mono"),
    )?;
    let mut options = UploadOptions {
        translation: translation_settings(
            &prefs,
            service,
            fields
                .get("lang_in")
                .cloned()
                .unwrap_or_else(|| "en".into()),
            fields
                .get("lang_out")
                .cloned()
                .unwrap_or_else(|| "zh-CN".into()),
        )?,
        pages: fields.get("page_range").cloned().unwrap_or_default(),
        format: Some(output_formats[0]),
        additional_formats: output_formats[1..].to_vec(),
        password: fields.get("password").cloned(),
        ..Default::default()
    };
    if options.translation.target == "zh" {
        options.translation.target = "zh-CN".into();
    }
    // Empty Google URL is significant to the existing translation cache key.
    enqueue(&s, &id, &filename, options)?;
    let mut public = snapshot(&s, &id);
    public["service"] = json!(service);
    private_json(&dir.join("settings.json"), &public)?;
    cleanup.keep = true;
    Ok(Json(job_value(&s, &s.get(&id)?)))
}
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
async fn job_events(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> WebResult<Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>>> {
    s.get(&id)?;
    let events = stream::unfold(
        (s, id, false, String::new()),
        |(s, id, done, previous)| async move {
            if done {
                return None;
            }
            let mut previous = previous;
            loop {
                let native = s.get(&id).ok()?;
                let job = job_value(&s, &native);
                let done = terminal(&native.status);
                let serialized = job.to_string();
                if serialized != previous || done {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()?
                        .as_micros() as u64;
                    SEQUENCE.fetch_max(now, Ordering::Relaxed);
                    let seq = SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
                    let data = json!({"seq":seq,"type":if done{job["status"].as_str().unwrap()}else{"progress"},"current":job["progress"]["current"],"total":job["progress"]["total"],"message":job["message"],"stage":job["stage"],"job":job,"data":{"files":job["files"],"error":job["error"]}});
                    let event = Event::default().id(seq.to_string()).json_data(data).ok()?;
                    return Some((Ok(event), (s, id, done, serialized)));
                }
                previous = serialized;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        },
    );
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}
fn paper_value(s: &Store, j: &JobInfo) -> Value {
    let job = job_value(s, j);
    json!({"id":j.id,"title":j.filename.trim_end_matches(".pdf"),"zhTitle":"","authors":"","pages":job["pages"],"size":format!("{:.1} MB",job["size"].as_u64().unwrap_or(0) as f64/1048576.),"service":job["options"]["service"].as_str().unwrap_or("Legacy task"),"date":iso_date(j.created).get(..10).unwrap_or(""),"status":job["status"],"downloadable":j.status=="completed"})
}
async fn papers(State(s): State<Shared>) -> WebResult<Json<Vec<Value>>> {
    let hidden = s.preferences.lock().unwrap().hidden.clone();
    Ok(Json(
        s.list()?
            .iter()
            .filter(|j| !hidden.contains(&j.id))
            .map(|j| paper_value(&s, j))
            .collect(),
    ))
}
async fn paper(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    Ok(Json(paper_value(&s, &s.get(&id)?)))
}
async fn content(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let j = s.get(&id)?;
    Ok(Json(
        json!({"id":id,"title":j.filename,"titleZh":"","authors":"","conference":"","totalPages":j.progress.map_or(0,|p|p.total),"abstract":{"en":"","zh":""},"sections":[],"has_content":false}),
    ))
}
async fn hide_paper(State(s): State<Shared>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    require(
        terminal(&s.get(&id)?.status),
        "cancel the running task before removing it from the library",
    )?;
    let mut p = s.preferences.lock().unwrap();
    let mut next = p.clone();
    next.hidden.insert(id);
    next.save(&s.root)?;
    *p = next;
    Ok(Json(json!({"deleted":true,"files_retained":true})))
}
async fn download(
    State(s): State<Shared>,
    Path((id, kind)): Path<(String, String)>,
    Query(query): Query<BTreeMap<String, String>>,
) -> WebResult<Response> {
    let j = s.get(&id)?;
    let path = if kind == "original" {
        s.root.join(&id).join("input.pdf")
    } else {
        require(
            j.status == "completed",
            "translation result is not available",
        )?;
        require(
            ["mono", "dual", "side_by_side"].contains(&kind.as_str()),
            "unknown PDF format",
        )?;
        let options = snapshot(&s, &id);
        let primary = options["output_formats"][0].as_str();
        if primary.is_none() || primary == Some(&kind) {
            s.root.join(&id).join("output.pdf")
        } else {
            s.root.join(&id).join(format!("output-{kind}.pdf"))
        }
    };
    let mut response = file_response(path, "application/pdf").await?;
    let disposition = if query.get("inline").is_some_and(|s| s == "true") {
        "inline"
    } else {
        "attachment"
    };
    // Keep persisted format keys compatible; downloads use the product labels.
    let filename_kind = match kind.as_str() {
        "side_by_side" => "dual",
        "dual" => "interleaved",
        other => other,
    };
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("{disposition}; filename=\"{id}-{filename_kind}.pdf\"").parse()?,
    );
    Ok(response)
}

pub(super) async fn static_file(
    State(s): State<Shared>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let route = uri.path();
    if route.starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let name = if route == "/" {
        "index.html"
    } else {
        route.trim_start_matches('/')
    };
    if FsPath::new(name)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let root = match tokio::fs::canonicalize(&s.web_root).await {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Frontend not built; run the documented static export build",
            )
                .into_response();
        }
    };
    let path = match tokio::fs::canonicalize(root.join(name)).await {
        Ok(p) if p.starts_with(&root) && p.is_file() => p,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let mime = match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        "xpi" => "application/x-xpinstall",
        _ => "application/octet-stream",
    };
    match file_response(path, mime).await {
        Ok(mut r) => {
            r.headers_mut().insert(
                header::CACHE_CONTROL,
                if route.starts_with("/_next/static/") {
                    header::HeaderValue::from_static("public, max-age=31536000, immutable")
                } else {
                    header::HeaderValue::from_static("no-cache")
                },
            );
            r
        }
        Err(e) => e.into_response(),
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
