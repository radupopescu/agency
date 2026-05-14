/// Config-driven REPL — reads provider settings from a TOML config file.
///
/// Create ~/.config/agency/config.toml:
///
///   [defaults]
///   provider = "local"
///
///   [providers.local]
///   base_url = "http://localhost:1234/v1"
///   default_model = "google/gemma-4-e4b"
///
/// Then run:
///   cargo run --example config
///
/// Or override individual values:
///   cargo run --example config -- --provider local --model "other/model"
use std::path::PathBuf;

use clap::Parser;

use agency::{
    config::Config,
    error::{Error, Result},
    providers::openai_compat::OpenAICompatProvider,
    repl::Repl,
};

#[derive(Parser)]
#[command(about = "M3 example: config-file-driven REPL")]
struct Args {
    #[arg(long, env = "AGENCY_CONFIG")]
    config: Option<PathBuf>,

    #[arg(long, env = "AGENCY_PROVIDER")]
    provider: Option<String>,

    #[arg(long, env = "AGENCY_MODEL")]
    model: Option<String>,

    #[arg(long, env = "AGENCY_SYSTEM")]
    system: Option<String>,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(args.config.as_deref())?;

    let provider_name = args
        .provider
        .as_deref()
        .or(config.defaults.provider.as_deref());
    let provider_cfg = provider_name.and_then(|n| config.provider(n));

    let base_url = provider_cfg
        .and_then(|p| p.base_url.clone())
        .unwrap_or_else(|| "http://localhost:1234/v1".into());

    let api_key = provider_cfg.and_then(|p| p.api_key.clone());

    let model = args
        .model
        .or_else(|| provider_cfg.and_then(|p| p.default_model.clone()))
        .ok_or_else(|| {
            Error::Config("no model specified; use --model or set default_model in config".into())
        })?;

    let provider = Box::new(OpenAICompatProvider::new(&base_url, api_key));
    let mut repl = Repl::new(provider, base_url, model, args.system);
    repl.run().await
}
