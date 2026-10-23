use super::*;
use crate::translate::Translator;
use axum::http::HeaderMap;

// Exercise the same profile -> protocol resolution used by multipart uploads,
// against a local HTTP server. No provider keys or paid services are required.
#[tokio::test(flavor = "current_thread")]
async fn every_profile_routes_its_own_credentials_and_protocol() -> Result<()> {
    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let requests = seen.clone();
    let app = Router::new().route(
        "/{*path}",
        post(
            move |OriginalUri(uri): OriginalUri, headers: HeaderMap, Json(body): Json<Value>| {
                let seen = requests.clone();
                async move {
                    let id = uri
                        .path()
                        .trim_start_matches('/')
                        .split('/')
                        .next()
                        .unwrap();
                    let spec = profile(id).unwrap();
                    let provider = spec["provider"].as_str().unwrap();
                    seen.lock().unwrap().insert(id.to_owned());
                    let key = format!("private-test-{id}");
                    if provider == "openai" {
                        assert!(uri.path().ends_with("/chat/completions"));
                        if id == "xinference" {
                            assert!(!headers.contains_key("authorization"));
                        } else {
                            assert_eq!(headers["authorization"], format!("Bearer {key}"));
                        }
                        assert_eq!(body["model"], "deployed-model");
                        assert!(
                            body["messages"][1]["content"]
                                .as_str()
                                .unwrap()
                                .contains("translation")
                        );
                    }
                    match provider {
                        "azureopenai" => {
                            assert_eq!(headers["api-key"], key);
                            assert!(
                                uri.path().ends_with(
                                    "/openai/deployments/deployed-model/chat/completions"
                                )
                            );
                            assert!(uri.query().unwrap().contains("api-version=2024-06-01"));
                        }
                        "azure" => {
                            assert_eq!(headers["ocp-apim-subscription-key"], key);
                            assert_eq!(headers["ocp-apim-subscription-region"], "westus");
                            assert!(uri.query().unwrap().contains("api-version=3.0"));
                            assert_eq!(body[0]["Text"], "A translation contract.");
                        }
                        "deepl" => {
                            assert_eq!(headers["authorization"], format!("DeepL-Auth-Key {key}"));
                            assert_eq!(body["text"][0], "A translation contract.");
                        }
                        "dify" => {
                            assert_eq!(headers["authorization"], format!("Bearer {key}"));
                            assert_eq!(body["inputs"]["text"], "A translation contract.");
                            assert_eq!(body["inputs"]["lang_out"], "zh-CN");
                        }
                        "anythingllm" => {
                            assert_eq!(headers["authorization"], format!("Bearer {key}"));
                            assert_eq!(body["mode"], "chat");
                        }
                        "ollama" => {
                            assert!(uri.path().ends_with("/api/chat"));
                            assert_eq!(body["stream"], false);
                        }
                        "deeplx" => {
                            assert!(uri.path().ends_with("/translate"));
                            assert_eq!(body["text"], "A translation contract.");
                        }
                        _ => {}
                    }
                    let text = "中文译文。";
                    Json(match provider {
                        "openai" | "azureopenai" => {
                            json!({"choices":[{"message":{"content":text}}]})
                        }
                        "ollama" => json!({"message":{"content":text}}),
                        "deepl" => json!({"translations":[{"text":text}]}),
                        "deeplx" => json!({"data":text}),
                        "azure" => json!([{"translations":[{"text":text}]}]),
                        "dify" => json!({"data":{"outputs":{"text":text}}}),
                        "anythingllm" => json!({"textResponse":text}),
                        _ => panic!("unexpected protocol"),
                    })
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let tmp = tempfile::tempdir()?;
    let mut prefs = Preferences::default();
    let specs = profiles();
    let mut expected = BTreeSet::new();
    for spec in specs.as_array().unwrap() {
        let id = spec["id"].as_str().unwrap();
        if id == "google" {
            continue;
        }
        expected.insert(id.to_owned());
        let mut config = json!({"base_url":format!("{base}/{id}"),"model":"deployed-model","options":{"region":"westus","api_version":"2024-06-01"}});
        if id != "xinference" {
            config["api_key"] = json!(format!("private-test-{id}"));
        }
        prefs.services.insert(id.into(), config);
        let settings = translation_settings(&prefs, id, "en".into(), "zh-CN".into())?;
        let mut translator = Translator::new(settings, &tmp.path().join(format!("{id}.sqlite3")))?;
        assert_eq!(
            translator.translate("A translation contract.").await?,
            "中文译文。"
        );
    }
    assert_eq!(*seen.lock().unwrap(), expected);
    let public = settings_value(&prefs).to_string();
    assert!(!public.contains("private-test-"));
    server.abort();
    Ok(())
}

#[test]
fn incomplete_profile_is_rejected_before_starting_a_worker() -> Result<()> {
    let mut prefs = Preferences::default();
    prefs.services.insert(
        "qwen".into(),
        json!({"api_key":"private-test","model":"deployed-model"}),
    );
    let error = translation_settings(&prefs, "qwen", "en".into(), "zh-CN".into())
        .unwrap_err()
        .to_string();
    assert!(error.contains("API URL"));
    assert!(!error.contains("private-test"));
    prefs
        .services
        .insert("gemini".into(), json!({"api_key":"private-test"}));
    assert!(
        translation_settings(&prefs, "gemini", "en".into(), "zh-CN".into())
            .unwrap_err()
            .to_string()
            .contains("model")
    );
    let old = translation_settings(&prefs, "google", "en".into(), "zh-CN".into())?;
    assert_eq!(old.provider, "google");
    assert!(old.base_url.is_empty());
    Ok(())
}
