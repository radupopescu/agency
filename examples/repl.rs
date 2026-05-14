/// Interactive multi-turn REPL.
///
/// Run with:
///   cargo run --example repl -- --model "google/gemma-4-e4b"
///
/// Or via env vars:
///   AGENCY_MODEL="google/gemma-4-e4b" cargo run --example repl
use clap::Parser;

use agency::{providers::openai_compat::OpenAICompatProvider, repl::Repl};

#[derive(Parser)]
#[command(about = "M2 example: multi-turn REPL with conversation history")]
struct Args {
    #[arg(
        long,
        env = "AGENCY_BASE_URL",
        default_value = "http://localhost:1234/v1"
    )]
    base_url: String,

    #[arg(long, env = "AGENCY_MODEL", default_value = "google/gemma-4-e4b")]
    model: String,

    #[arg(long, env = "AGENCY_API_KEY")]
    api_key: Option<String>,

    #[arg(long, env = "AGENCY_SYSTEM")]
    system: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let provider = Box::new(OpenAICompatProvider::new(&args.base_url, args.api_key));
    let mut repl = Repl::new(provider, args.base_url, args.model, args.system);

    if let Err(e) = repl.run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
