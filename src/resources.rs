use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub fn cache() -> PathBuf {
    if let Some(p) = std::env::var_os("PDF2ZH_ASSETS") {
        return p.into();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let assets = parent.join("assets");
            if assets.join("fonts").is_dir() {
                return assets;
            }
        }
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache/babeldoc")
}

pub fn font_path() -> Result<PathBuf> {
    let path = std::env::var_os("PDF2ZH_FONT")
        .map(PathBuf::from)
        .unwrap_or_else(|| cache().join("fonts/SourceHanSerifCN-Regular.ttf"));
    if !path.is_file() {
        bail!(
            "Chinese font missing: {}. Set PDF2ZH_FONT to a TrueType/OpenType font.",
            path.display()
        );
    }
    Ok(path)
}
pub fn font_bytes() -> Result<Vec<u8>> {
    std::fs::read(font_path()?).context("reading output font")
}
pub fn model_path() -> PathBuf {
    cache().join("models/doclayout_yolo_docstructbench_imgsz1024.onnx")
}

pub fn runtime_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ORT_DYLIB_PATH") {
        return Some(p.into());
    }
    let name = if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    let mut candidates = vec![PathBuf::from("assets").join(name)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("lib").join(name));
            candidates.push(dir.join("assets").join(name));
            candidates.push(dir.join("../../assets").join(name));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

#[derive(serde::Deserialize)]
struct Manifest {
    assets: Vec<Asset>,
}
#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    directory: String,
    sha3_256: String,
    url: String,
}

fn manifest() -> Result<Manifest> {
    Ok(serde_json::from_str(include_str!(
        "../assets/manifest.json"
    ))?)
}

pub fn sha3(path: &std::path::Path) -> Result<String> {
    use sha3::{Digest, Sha3_256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha3_256::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}

pub fn status() -> Result<serde_json::Value> {
    let mut assets = Vec::new();
    for asset in manifest()?.assets {
        let path = cache().join(&asset.directory).join(&asset.name);
        let valid = sha3(&path).is_ok_and(|h| h == asset.sha3_256);
        assets.push(serde_json::json!({"name":asset.name,"path":path,"exists":path.is_file(),"valid":valid}));
    }
    Ok(
        serde_json::json!({"assets":assets,"native_runtime":runtime_path(),"python_runtime_required":false}),
    )
}

/// Explicit preparation only. Neither serve nor worker downloads assets.
pub async fn prepare(output: &std::path::Path, download: bool) -> Result<()> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    std::fs::create_dir_all(output)?;
    for asset in manifest()?.assets {
        let source = cache().join(&asset.directory).join(&asset.name);
        let target = output.join(&asset.directory).join(&asset.name);
        std::fs::create_dir_all(target.parent().unwrap())?;
        if sha3(&target).is_ok_and(|h| h == asset.sha3_256) {
            continue;
        }
        if sha3(&source).is_ok_and(|h| h == asset.sha3_256) {
            if source != target {
                std::fs::copy(&source, &target)?;
            }
        } else {
            anyhow::ensure!(
                download,
                "missing or corrupt resource {}; run resources prepare --download explicitly",
                asset.name
            );
            let temp = target.with_extension("partial");
            let response = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()?
                .get(&asset.url)
                .send()
                .await?
                .error_for_status()?;
            let mut stream = response.bytes_stream();
            let mut file = tokio::fs::File::create(&temp).await?;
            let mut size = 0usize;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                size += chunk.len();
                anyhow::ensure!(size <= 512 * 1024 * 1024, "resource exceeds size limit");
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            drop(file);
            if sha3(&temp)? != asset.sha3_256 {
                let _ = std::fs::remove_file(&temp);
                anyhow::bail!("resource hash mismatch: {}", asset.name);
            }
            std::fs::rename(temp, &target)?;
        }
        anyhow::ensure!(
            sha3(&target)? == asset.sha3_256,
            "resource verification failed: {}",
            asset.name
        );
    }
    let runtime = runtime_path().context("native ONNX Runtime missing; set ORT_DYLIB_PATH")?;
    let filename = if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    let target = output.join(filename);
    if std::fs::canonicalize(&runtime).ok() != std::fs::canonicalize(&target).ok() {
        std::fs::copy(runtime, target)?;
    }
    std::fs::write(
        output.join("manifest.json"),
        include_str!("../assets/manifest.json"),
    )?;
    Ok(())
}
