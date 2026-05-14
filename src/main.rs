use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use agency::{
    config::Config,
    error::{Error, Result},
    provider::LlmProvider,
    providers::openai_compat::OpenAICompatProvider,
    repl::Repl,
};

#[derive(Parser)]
#[command(
    name = "agency",
    version,
    about = "An LLM agent for learning and experimentation"
)]
struct Cli {
    /// Provider name from config (e.g. "local", "apertus")
    #[arg(short, long)]
    provider: Option<String>,

    /// Model name — overrides config default_model
    #[arg(short, long)]
    model: Option<String>,

    /// OpenAI-compatible API base URL — overrides config base_url
    #[arg(long)]
    base_url: Option<String>,

    /// API key — overrides config api_key
    #[arg(long)]
    api_key: Option<String>,

    /// System prompt
    #[arg(short, long)]
    system: Option<String>,

    /// Path to config file (default: ~/.config/agency/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,

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

async fn run() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let config = Config::load(cli.config.as_deref())?;

    // Resolve provider section: --provider > config defaults.provider
    let provider_name = cli
        .provider
        .as_deref()
        .or(config.defaults.provider.as_deref());
    let provider_cfg = provider_name.and_then(|n| config.provider(n));

    // Merge: CLI flag > provider config > built-in default
    let base_url = cli
        .base_url
        .or_else(|| provider_cfg.and_then(|p| p.base_url.clone()))
        .unwrap_or_else(|| "http://localhost:1234/v1".into());

    let api_key = cli
        .api_key
        .or_else(|| provider_cfg.and_then(|p| p.api_key.clone()));

    let model = cli
        .model
        .or_else(|| provider_cfg.and_then(|p| p.default_model.clone()))
        .ok_or_else(|| {
            Error::Config("no model specified; use --model or set default_model in config".into())
        })?;

    let provider: Box<dyn LlmProvider> = Box::new(OpenAICompatProvider::new(&base_url, api_key));

    if let Some(prompt) = cli.prompt {
        // Single-turn mode
        use agency::{
            message::{ContentBlock, Message, Role},
            provider::CompletionRequest,
            stream::StreamEvent,
        };
        use futures::StreamExt;
        use std::io::{self, Write};

        let request = CompletionRequest {
            model,
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
        let mut repl = Repl::new(provider, base_url, model, cli.system);
        repl.run().await?;
    }

    Ok(())
}
