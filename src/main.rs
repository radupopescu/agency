use std::io::{self, Write};

use clap::Parser;
use futures::StreamExt;
use tracing_subscriber::EnvFilter;

use agency::{
    message::{ContentBlock, Message, Role},
    provider::{CompletionRequest, LlmProvider},
    providers::openai_compat::OpenAICompatProvider,
    stream::StreamEvent,
};

#[derive(Parser)]
#[command(
    name = "agency",
    version,
    about = "An LLM agent for learning and experimentation"
)]
struct Cli {
    /// Model name as shown in LM Studio (e.g. "google/gemma-4-e4b")
    #[arg(short, long)]
    model: String,

    /// OpenAI-compatible API base URL
    #[arg(long, default_value = "http://localhost:1234/v1")]
    base_url: String,

    /// Optional API key
    #[arg(long)]
    api_key: Option<String>,

    /// Prompt to send
    prompt: String,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> agency::error::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let provider = OpenAICompatProvider::new(&cli.base_url, cli.api_key);

    let request = CompletionRequest {
        model: cli.model,
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: cli.prompt }],
        }],
        system: None,
        temperature: None,
        max_tokens: None,
    };

    let mut stream = provider.stream(request).await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta(text) => {
                print!("{text}");
                io::stdout().flush()?;
            }
            StreamEvent::MessageEnd { .. } => break,
            _ => {}
        }
    }
    println!();

    Ok(())
}
