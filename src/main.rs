use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "agency",
    version,
    about = "An LLM agent for learning and experimentation"
)]
struct Cli {}

fn main() {
    Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("agency M0: skeleton — no LLM wired up yet");
}
