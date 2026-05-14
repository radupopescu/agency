use clap::Parser;
use tracing_subscriber::EnvFilter;

use agency::{provider::LlmProvider, providers::openai_compat::OpenAICompatProvider, repl::Repl};

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

    /// Optional system prompt
    #[arg(short, long)]
    system: Option<String>,

    /// Single-turn prompt. Omit to start the interactive REPL.
    prompt: Option<String>,
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

    let provider: Box<dyn LlmProvider> =
        Box::new(OpenAICompatProvider::new(&cli.base_url, cli.api_key));

    if let Some(prompt) = cli.prompt {
        // Single-turn mode (M1 behaviour)
        use agency::{
            message::{ContentBlock, Message, Role},
            provider::CompletionRequest,
            stream::StreamEvent,
        };
        use futures::StreamExt;
        use std::io::{self, Write};

        let request = CompletionRequest {
            model: cli.model,
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text { text: prompt }],
            }],
            system: cli.system,
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
    } else {
        // Interactive REPL mode
        let mut repl = Repl::new(provider, cli.model, cli.system);
        repl.run().await?;
    }

    Ok(())
}
