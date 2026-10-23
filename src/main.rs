use anyhow::Result;
use clap::{Parser, Subcommand};
use pdf2zh_rust::{pdf, resources};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Native PDF translation with bounded task processes")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Resources {
        #[command(subcommand)]
        command: ResourceCommand,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1:7861")]
        listen: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
    },
    #[command(hide = true)]
    Worker {
        #[arg(long)]
        job: PathBuf,
    },
    #[command(hide = true)]
    Analyze {
        #[arg(long)]
        request: PathBuf,
    },
    Inspect {
        input: PathBuf,
        #[arg(long, default_value_t = 1)]
        page: usize,
    },
    Render {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value_t = 1)]
        page: usize,
        #[arg(long, default_value_t = 96.)]
        dpi: f32,
    },
    Fixture {
        output: PathBuf,
    },
    Layout {
        input: PathBuf,
        #[arg(long, default_value_t = 1)]
        page: usize,
        #[arg(long)]
        model: Option<PathBuf>,
    },
    Translate {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        pages: Option<String>,
        #[arg(long)]
        no_layout: bool,
        #[arg(long, value_enum)]
        format: Option<pdf2zh_rust::engine::OutputFormat>,
    },
    Probe {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value = "你好，世界。这是 Rust 写回的中文译文。")]
        text: String,
    },
}

#[derive(Subcommand)]
enum ResourceCommand {
    Status,
    Prepare {
        #[arg(long, default_value = "assets")]
        output: PathBuf,
        #[arg(long)]
        download: bool,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Resources { command } => match command {
            ResourceCommand::Status => {
                println!("{}", serde_json::to_string_pretty(&resources::status()?)?)
            }
            ResourceCommand::Prepare { output, download } => {
                resources::prepare(&output, download).await?
            }
        },
        Command::Serve { listen, data_dir } => {
            pdf2zh_rust::service::serve(data_dir, listen).await?
        }
        Command::Worker { job } => pdf2zh_rust::service::worker(job).await?,
        Command::Analyze { request } => pdf2zh_rust::analysis::analyze(&request)?,
        Command::Inspect { input, page } => {
            let doc = pdf::open(&input, None)?;
            let info = pdf::page_text(
                &doc,
                page.checked_sub(1)
                    .ok_or_else(|| anyhow::anyhow!("page numbers start at 1"))?,
            )?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Command::Render {
            input,
            output,
            page,
            dpi,
        } => pdf::render(
            &input,
            &output,
            page.checked_sub(1)
                .ok_or_else(|| anyhow::anyhow!("page numbers start at 1"))?,
            dpi,
        )?,
        Command::Fixture { output } => pdf::fixture(&output)?,
        Command::Layout { input, page, model } => {
            let doc = pdf::open(&input, None)?;
            let page = doc.load_pdf_page(
                page.checked_sub(1)
                    .ok_or_else(|| anyhow::anyhow!("page numbers start at 1"))?
                    .try_into()?,
            )?;
            let bounds = page.bounds()?.into();
            let raster = pdf::render_rgb(&page, 1.)?;
            let mut model = pdf2zh_rust::layout::LayoutModel::load(
                &model.unwrap_or_else(resources::model_path),
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&model.predict(&raster, bounds)?)?
            );
        }
        Command::Probe {
            input,
            output,
            text,
        } => pdf::probe(&input, &output, &text, &resources::font_bytes()?)?,
        Command::Translate {
            input,
            output,
            config,
            provider,
            source,
            target,
            pages,
            no_layout,
            format,
        } => {
            let mut job = if let Some(path) = config {
                serde_json::from_slice::<pdf2zh_rust::engine::JobConfig>(&std::fs::read(path)?)?
            } else {
                pdf2zh_rust::engine::JobConfig::default()
            };
            job.input = input;
            job.output = output;
            if let Some(provider) = provider {
                job.translation.provider = provider;
            }
            if let Some(source) = source {
                job.translation.source = source;
            }
            if let Some(target) = target {
                job.translation.target = target;
            }
            if let Some(pages) = pages {
                job.pages = pages;
            }
            if no_layout {
                job.layout = false;
            }
            if let Some(format) = format {
                job.format = format;
            }
            pdf2zh_rust::engine::run(&job, |event| {
                eprintln!("{}", serde_json::to_string(&event).unwrap())
            })
            .await?;
        }
    }
    Ok(())
}
