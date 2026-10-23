//! A bounded native analysis stage. Model/runtime allocations belong only to
//! this process, rather than staying resident through remote translation waits.
use crate::{
    engine::{JobConfig, Progress, page_selection},
    layout::LayoutModel,
    pdf, resources,
    types::LayoutRegion,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Serialize, Deserialize)]
struct Request {
    input: PathBuf,
    password: Option<String>,
    pages: String,
    output: PathBuf,
    cancel: PathBuf,
}

pub async fn run_process(
    job: &JobConfig,
    emit: &mut impl FnMut(Progress),
) -> Result<BTreeMap<usize, Vec<LayoutRegion>>> {
    let id = uuid::Uuid::new_v4();
    let request_path = job.data_dir.join(format!("analysis-{id}.json"));
    let output_path = job.data_dir.join(format!("analysis-{id}.result.json"));
    let request = Request {
        input: job.input.clone(),
        password: job.password.clone(),
        pages: job.pages.clone(),
        output: output_path.clone(),
        cancel: job.data_dir.join("cancel"),
    };
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(&request_path)?
        .write_all(&serde_json::to_vec(&request)?)?;
    let result = async {
        let mut child = tokio::process::Command::new(std::env::current_exe()?)
            .arg("analyze")
            .arg("--request")
            .arg(&request_path)
            .env("PDF2ZH_PARENT_PID", std::process::id().to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().context("missing analyzer output")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break;
            }
            ensure!(n < 64 * 1024, "analyzer event too large");
            emit(serde_json::from_str(&line)?);
        }
        ensure!(
            child.wait().await?.success(),
            "native layout analysis failed"
        );
        ensure!(
            std::fs::metadata(&output_path)?.len() < 32 * 1024 * 1024,
            "layout metadata exceeds limit"
        );
        Ok::<_, anyhow::Error>(serde_json::from_slice(&std::fs::read(&output_path)?)?)
    }
    .await;
    for path in [request_path, output_path] {
        let _ = std::fs::remove_file(path);
    }
    result
}

pub fn analyze(request_path: &Path) -> Result<()> {
    guard_parent()?;
    let request: Request = serde_json::from_slice(&std::fs::read(request_path)?)?;
    let doc = pdf::open(&request.input, request.password.as_deref())?;
    let total = doc.page_count()? as usize;
    let selected = page_selection(&request.pages, total)?;
    let mut model = LayoutModel::load(&resources::model_path())?;
    let mut result = BTreeMap::new();
    for index in selected {
        check_parent()?;
        ensure!(
            !request.cancel.exists(),
            "translation cancelled during layout analysis"
        );
        let mut page = doc.load_pdf_page(index as i32)?;
        if page.rotation()? != 0 {
            page.set_rotation(0)?;
            page = doc.reload_page(index)?;
        }
        println!(
            "{}",
            serde_json::to_string(&Progress {
                stage: "layout".into(),
                page: index + 1,
                total,
                completed_blocks: 0,
                message: String::new()
            })?
        );
        let image = pdf::render_rgb(&page, 1.)?;
        result.insert(index, model.predict(&image, page.bounds()?.into())?);
        check_parent()?;
        ensure!(
            !request.cancel.exists(),
            "translation cancelled during layout analysis"
        );
    }
    std::fs::write(request.output, serde_json::to_vec(&result)?)?;
    Ok(())
}

pub fn guard_parent() -> Result<()> {
    if std::env::var_os("PDF2ZH_PARENT_PID").is_none() {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    ensure!(
        unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) } == 0,
        "failed to register worker parent-death signal"
    );
    check_parent()
}

fn check_parent() -> Result<()> {
    #[cfg(unix)]
    if let Ok(parent) = std::env::var("PDF2ZH_PARENT_PID") {
        ensure!(
            parent.parse::<u32>()? == unsafe { libc::getppid() } as u32,
            "layout task parent exited"
        );
    }
    Ok(())
}
