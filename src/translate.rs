use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationSettings {
    pub provider: String,
    pub source: String,
    pub target: String,
    pub base_url: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub glossary: BTreeMap<String, String>,
    pub requests_per_second: f64,
    pub options: BTreeMap<String, String>,
}
impl Default for TranslationSettings {
    fn default() -> Self {
        Self {
            provider: "google".into(),
            source: "en".into(),
            target: "zh-CN".into(),
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            glossary: BTreeMap::new(),
            requests_per_second: 2.,
            options: BTreeMap::new(),
        }
    }
}

pub struct Translator {
    settings: TranslationSettings,
    client: reqwest::Client,
    cache: Connection,
    last_request: Option<std::time::Instant>,
    session: String,
    cache_only: bool,
}

#[derive(Debug)]
struct ProviderError {
    status: reqwest::StatusCode,
    detail: String,
}
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "translation service returned HTTP {}{}",
            self.status.as_u16(),
            self.detail
        )
    }
}
impl std::error::Error for ProviderError {}

fn retryable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ProviderError>().is_none_or(|e| {
        e.status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || e.status == reqwest::StatusCode::REQUEST_TIMEOUT
            || e.status.is_server_error()
    })
}

// Keep structured provider diagnostics, never a raw HTML error page or request
// dump. Bound the response and remove supplied credentials before public history.
async fn check_response(
    mut response: reqwest::Response,
    key: Option<&str>,
    text: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let mut bytes = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        if bytes.len() + chunk.len() > 16 * 1024 {
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut detail = String::new();
    if let Ok(body) = serde_json::from_slice::<Value>(&bytes) {
        let error = body.get("error").unwrap_or(&body);
        for field in ["type", "code", "message"] {
            if let Some(value) = error.get(field).and_then(Value::as_str) {
                if !detail.is_empty() {
                    detail.push_str("; ");
                }
                detail.push_str(value);
            }
        }
        if detail.is_empty()
            && let Some(value) = error.as_str()
        {
            detail.push_str(value);
        }
    }
    if let Some(key) = key.filter(|s| !s.is_empty()) {
        detail = detail.replace(key, "[redacted]");
    }
    if !text.is_empty() {
        detail = detail.replace(text, "[request text]");
    }
    let secrets = regex::Regex::new(r"(?i)(?:bearer\s+|sk[-_])[A-Za-z0-9._~+/=-]+").unwrap();
    detail = secrets.replace_all(&detail, "[redacted]").into_owned();
    detail = detail
        .chars()
        .filter(|c| !c.is_control())
        .take(600)
        .collect();
    if !detail.is_empty() {
        detail.insert_str(0, ": ");
    }
    Err(ProviderError { status, detail }.into())
}
impl Translator {
    pub fn new(mut settings: TranslationSettings, cache: &Path) -> Result<Self> {
        ensure!(
            settings.requests_per_second.is_finite() && settings.requests_per_second > 0.,
            "requests_per_second must be positive"
        );
        ensure!(
            !settings.target.trim().is_empty(),
            "target language is required"
        );
        ensure!(
            [
                "google",
                "bing",
                "openai",
                "ollama",
                "deepl",
                "deeplx",
                "azure",
                "azureopenai",
                "dify",
                "anythingllm"
            ]
            .contains(&settings.provider.as_str()),
            "provider {} is not yet implemented",
            settings.provider
        );
        if settings.api_key.is_none() {
            settings.api_key = std::env::var("PDF2ZH_API_KEY").ok();
        }
        let cache = Connection::open(cache)?;
        cache.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS translations (key TEXT PRIMARY KEY, value TEXT NOT NULL, accessed INTEGER NOT NULL DEFAULT (unixepoch()));")?;
        let client = reqwest::Client::builder()
            .user_agent(concat!("pdf2zh-rust/", env!("CARGO_PKG_VERSION")))
            .cookie_store(true)
            .timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            settings,
            client,
            cache,
            last_request: None,
            session: uuid::Uuid::new_v4().to_string(),
            cache_only: std::env::var_os("PDF2ZH_CACHE_ONLY").is_some(),
        })
    }

    pub async fn translate(&mut self, original: &str) -> Result<String> {
        if original.trim().is_empty() || !original.chars().any(char::is_alphabetic) {
            return Ok(original.to_owned());
        }
        let key = hex::encode(Sha256::digest(serde_json::to_vec(
            &json!({"v":2,"text":original,"provider":self.settings.provider,
            "source":self.settings.source,"target":self.settings.target,"url":self.settings.base_url,
            "model":self.settings.model,"glossary":self.settings.glossary,"options":self.settings.options}),
        )?));
        if let Ok(value) =
            self.cache
                .query_row("SELECT value FROM translations WHERE key=?1", [&key], |r| {
                    r.get::<_, String>(0)
                })
        {
            if crate::typeset::parse(&value, original.matches('⟦').count()).is_ok() {
                return Ok(value);
            }
            self.cache
                .execute("DELETE FROM translations WHERE key=?1", [&key])?;
        }
        ensure!(
            !self.cache_only,
            "translation is not cached; cache-only recovery makes no network requests"
        );
        let mut text = original.to_owned();
        let mut terms = self.settings.glossary.iter().collect::<Vec<_>>();
        terms.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        for (source, target) in terms {
            if !source.is_empty() {
                text = text.replace(source, target);
            }
        }
        let mut output = None;
        for attempt in 0..3 {
            let interval = Duration::try_from_secs_f64(1. / self.settings.requests_per_second)
                .context("configured request interval is too large")?;
            if let Some(last) = self.last_request
                && let Some(wait) = interval.checked_sub(last.elapsed())
            {
                tokio::time::sleep(wait).await;
            }
            self.last_request = Some(std::time::Instant::now());
            match self.request(&text).await {
                Ok(v) => match crate::typeset::parse(&v, original.matches('⟦').count()) {
                    Ok(_) => {
                        output = Some(v);
                        break;
                    }
                    Err(e) if attempt == 2 => return Err(e),
                    Err(_) => tokio::time::sleep(Duration::from_secs(1 << attempt)).await,
                },
                Err(e) if attempt == 2 || !retryable(&e) => return Err(e),
                Err(_) => tokio::time::sleep(Duration::from_secs(1 << attempt)).await,
            }
        }
        let output = output.context("translation request produced no result")?;
        ensure!(!output.trim().is_empty(), "translation returned empty text");
        ensure!(
            output.chars().count() <= original.chars().count().saturating_mul(20).max(2000),
            "translation response is unexpectedly long"
        );
        self.cache.execute(
            "INSERT OR REPLACE INTO translations(key,value) VALUES (?1,?2)",
            params![key, output],
        )?;
        self.cache.execute("DELETE FROM translations WHERE key IN (SELECT key FROM translations ORDER BY accessed DESC LIMIT -1 OFFSET 50000)",[])?;
        Ok(output)
    }

    async fn request(&self, text: &str) -> Result<String> {
        if self.settings.provider == "bing" {
            return self.bing(text).await;
        }
        self.request_one(text).await
    }

    async fn bing(&self, text: &str) -> Result<String> {
        let endpoint = if self.settings.base_url.is_empty() {
            "https://www.bing.com/translator"
        } else {
            &self.settings.base_url
        };
        let response = self
            .client
            .get(endpoint)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Bing session request failed"))?;
        ensure!(
            response.status().is_success(),
            "Bing session returned HTTP {}",
            response.status().as_u16()
        );
        let mut base = response.url().clone();
        base.set_path("/ttranslatev3");
        base.set_query(None);
        let html = response.text().await?;
        let capture = |pattern: &str| -> Result<String> {
            Ok(regex::Regex::new(pattern)?
                .captures(&html)
                .and_then(|c| c.get(1))
                .context("Bing page protocol changed: missing session field")?
                .as_str()
                .to_owned())
        };
        let ig =
            capture(r#"(?i)\bIG\s*[:=]\s*"([^"]+)""#).or_else(|_| capture(r#""ig":"([^"]+)""#))?;
        let iid = regex::Regex::new(r#"data-iid="([^"]+)""#)?
            .captures_iter(&html)
            .last()
            .and_then(|c| c.get(1))
            .context("Bing page protocol changed: missing translator identifier")?
            .as_str()
            .to_owned();
        let auth =
            regex::Regex::new(r#"params_AbusePreventionHelper\s*=\s*\[\s*(\d+),\s*"([^"]+)""#)?;
        let auth = auth
            .captures(&html)
            .context("Bing page protocol changed: missing request token")?;
        let source = bing_language(&self.settings.source);
        let target = bing_language(&self.settings.target);
        let mut output = String::new();
        for chunk in chunks(text, 1000) {
            let response = self
                .client
                .post(base.clone())
                .query(&[("IG", ig.as_str()), ("IID", iid.as_str())])
                .form(&[
                    ("fromLang", source.as_str()),
                    ("to", target.as_str()),
                    ("text", chunk),
                    ("key", &auth[1]),
                    ("token", &auth[2]),
                ])
                .header("User-Agent", "Mozilla/5.0")
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("Bing translation request failed"))?;
            ensure!(
                response.status().is_success(),
                "Bing translation returned HTTP {}",
                response.status().as_u16()
            );
            let value: Value = response.json().await?;
            output.push_str(
                value[0]["translations"][0]["text"]
                    .as_str()
                    .context("invalid Bing translation response")?,
            );
        }
        Ok(output)
    }

    async fn request_one(&self, text: &str) -> Result<String> {
        let s = &self.settings;
        let response=match s.provider.as_str() {
            "google" => self.client.get("https://translate.googleapis.com/translate_a/single")
                .query(&[("client","gtx"),("dt","t"),("sl",s.source.as_str()),("tl",s.target.as_str()),("q",text)]).send().await,
            "openai" => {
                ensure!(!s.base_url.is_empty() && !s.model.is_empty(),"OpenAI-compatible base_url and model are required");
                let mut request=self.client.post(format!("{}/chat/completions",s.base_url.trim_end_matches('/')))
                    .header("x-opencode-session", &self.session)
                    .json(&json!({"model":s.model,"temperature":0,"messages":[
                        {"role":"system","content":format!("Translate from {} to {}. Return only the translation, no explanations, no markdown fences. Preserve all symbols and technical terminology. Preserve every ⟦number⟧ marker exactly once, without changing its number.",s.source,s.target)},
                        {"role":"user","content":text}]}));
                if let Some(key)=s.api_key.as_deref().filter(|key|!key.is_empty()) { request=request.bearer_auth(key); }
                request.send().await
            },
            "ollama" => {
                ensure!(!s.model.is_empty(),"Ollama model is required");
                let url=if s.base_url.is_empty() { "http://127.0.0.1:11434" } else { &s.base_url };
                self.client.post(format!("{}/api/chat",url.trim_end_matches('/'))).json(&json!({"model":s.model,"stream":false,
                    "messages":[{"role":"system","content":format!("Translate from {} to {}. Output only translated text.",s.source,s.target)},
                    {"role":"user","content":text}]})).send().await
            },
            "deepl" => {
                let key=s.api_key.as_ref().context("DeepL API key is required")?;
                let url=if s.base_url.is_empty() { if key.ends_with(":fx") {"https://api-free.deepl.com/v2"} else {"https://api.deepl.com/v2"} } else {&s.base_url};
                self.client.post(format!("{}/translate",url.trim_end_matches('/'))).header("Authorization",format!("DeepL-Auth-Key {key}"))
                    .json(&json!({"text":[text],"source_lang":s.source.to_uppercase(),"target_lang":s.target.split('-').next().unwrap_or(&s.target).to_uppercase()})).send().await
            },
            "deeplx" => {
                ensure!(!s.base_url.is_empty(),"DeepLX base_url is required");
                self.client.post(format!("{}/translate",s.base_url.trim_end_matches('/')))
                    .json(&json!({"text":text,"source_lang":s.source,"target_lang":s.target})).send().await
            },
            "azure"=>{
                let key=s.api_key.as_ref().context("Azure Translator API key is required")?;
                ensure!(!s.base_url.is_empty(),"Azure Translator endpoint is required");
                self.client.post(format!("{}/translate",s.base_url.trim_end_matches('/')))
                    .query(&[("api-version","3.0"),("from",s.source.as_str()),("to",s.target.as_str())])
                    .header("Ocp-Apim-Subscription-Key",key)
                    .header("Ocp-Apim-Subscription-Region",s.options.get("region").map(String::as_str).unwrap_or("chinaeast2"))
                    .json(&json!([{"Text":text}])).send().await
            },
            "azureopenai"=>{
                let key=s.api_key.as_ref().context("Azure OpenAI API key is required")?;
                ensure!(!s.base_url.is_empty() && !s.model.is_empty(),"Azure endpoint and deployment model are required");
                self.client.post(format!("{}/openai/deployments/{}/chat/completions",s.base_url.trim_end_matches('/'),s.model))
                    .query(&[("api-version",s.options.get("api_version").map(String::as_str).unwrap_or("2024-10-21"))])
                    .header("api-key",key).json(&json!({"messages":[{"role":"system","content":format!("Translate from {} to {}. Return only translation. Preserve every ⟦number⟧ marker exactly once.",s.source,s.target)},
                    {"role":"user","content":text}]})).send().await
            },
            "dify"|"anythingllm"=>{
                let key=s.api_key.as_ref().context("service API key is required")?;
                ensure!(!s.base_url.is_empty(),"service URL is required");
                let body=if s.provider=="dify" {json!({"inputs":{"lang_in":s.source,"lang_out":s.target,"text":text},"response_mode":"blocking","user":"pdf2zh-rust"})}
                else {json!({"message":format!("Translate from {} to {}. Preserve every ⟦number⟧ marker and return only translation.\n{}",s.source,s.target,text),"mode":"chat","sessionId":uuid::Uuid::new_v4().to_string()})};
                self.client.post(&s.base_url).bearer_auth(key).json(&body).send().await
            },
            _ => bail!("unsupported provider"),
        }.map_err(|_|anyhow::anyhow!("translation network request failed"))?;
        let response = check_response(response, s.api_key.as_deref(), text).await?;
        let body: Value = response
            .json()
            .await
            .context("invalid translation response JSON")?;
        match s.provider.as_str() {
            "google" => {
                let segments = body[0]
                    .as_array()
                    .context("invalid Google translation response")?;
                Ok(segments
                    .iter()
                    .filter_map(|x| x[0].as_str())
                    .collect::<String>())
            }
            "openai" | "azureopenai" => Ok(body["choices"][0]["message"]["content"]
                .as_str()
                .context("missing translated message")?
                .trim()
                .to_owned()),
            "ollama" => Ok(body["message"]["content"]
                .as_str()
                .context("missing Ollama response")?
                .trim()
                .to_owned()),
            "deepl" => Ok(body["translations"][0]["text"]
                .as_str()
                .context("missing DeepL response")?
                .to_owned()),
            "deeplx" => Ok(body["data"]
                .as_str()
                .context("missing DeepLX response")?
                .to_owned()),
            "azure" => Ok(body[0]["translations"][0]["text"]
                .as_str()
                .context("missing Azure translation")?
                .to_owned()),
            "dify" => Ok(body["data"]["outputs"]["text"]
                .as_str()
                .context("missing Dify workflow output")?
                .to_owned()),
            "anythingllm" => Ok(body["textResponse"]
                .as_str()
                .context("missing AnythingLLM response")?
                .trim()
                .to_owned()),
            _ => bail!("unsupported response"),
        }
    }
}

fn bing_language(language: &str) -> String {
    match language.to_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-hans" => "zh-Hans".into(),
        "zh-tw" | "zh-hk" | "zh-hant" => "zh-Hant".into(),
        "auto" => "auto-detect".into(),
        _ => language.into(),
    }
}

fn chunks(text: &str, max_chars: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut remaining = text;
    while remaining.chars().count() > max_chars {
        let mut end = remaining.char_indices().nth(max_chars).unwrap().0;
        if let Some(mark) = remaining[..end].rfind('⟦')
            && !remaining[mark..end].contains('⟧')
        {
            end = mark;
        }
        if end == 0 {
            end = remaining.char_indices().nth(max_chars).unwrap().0;
        }
        chunks.push(&remaining[..end]);
        remaining = &remaining[end..];
    }
    if !remaining.is_empty() {
        chunks.push(remaining);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cache_only_recovery_makes_no_new_provider_requests() -> Result<()> {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let app = axum::Router::new().route(
            "/chat/completions",
            axum::routing::post(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                async { axum::Json(json!({"choices":[{"message":{"content":"中文译文"}}]})) }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let tmp = tempfile::tempdir()?;
        let mut translator = Translator::new(
            TranslationSettings {
                provider: "openai".into(),
                base_url: url,
                model: "cache-contract".into(),
                ..Default::default()
            },
            &tmp.path().join("cache.sqlite3"),
        )?;
        assert_eq!(
            translator.translate("original paragraph").await?,
            "中文译文"
        );
        translator.cache_only = true;
        assert_eq!(
            translator.translate("original paragraph").await?,
            "中文译文"
        );
        assert!(
            translator
                .translate("uncached paragraph")
                .await
                .unwrap_err()
                .to_string()
                .contains("cache-only")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn provider_errors_are_bounded_redacted_and_retry_only_transient_statuses() -> Result<()>
    {
        use axum::{Json, Router, http::StatusCode, routing::post};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let app = Router::new()
            .route("/permanent", post(|| async { (StatusCode::BAD_REQUEST, Json(json!({"error":{"code":"bad_model","message":"missing session; secret-key-example; private input"}}))) }))
            .route("/transient", post(|| async { (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error":{"message":"rate limited"}}))) }))
            .route("/large", post(|| async { (StatusCode::BAD_REQUEST, "x".repeat(32 * 1024)) }));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::new();
        let error = check_response(
            client.post(format!("{url}/permanent")).send().await?,
            Some("secret-key-example"),
            "private input",
        )
        .await
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("bad_model") && message.contains("missing session"));
        assert!(!message.contains("secret-key-example") && !message.contains("private input"));
        assert!(!retryable(&error));
        let error = check_response(
            client.post(format!("{url}/transient")).send().await?,
            None,
            "",
        )
        .await
        .unwrap_err();
        assert!(retryable(&error));
        let error = check_response(client.post(format!("{url}/large")).send().await?, None, "")
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "translation service returned HTTP 400");
        server.abort();
        Ok(())
    }

    #[test]
    fn bounded_chunks_preserve_utf8_and_formula_markers() {
        let text = "你好世界⟦123⟧，测试结束。";
        let parts = chunks(text, 7);
        assert_eq!(parts.concat(), text);
        assert!(
            parts
                .iter()
                .all(|p| p.matches('⟦').count() == p.matches('⟧').count())
        );
    }
}
