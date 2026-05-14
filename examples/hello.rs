/// Run with:
///   cargo run --example hello -- --base-url http://localhost:1234/v1 \
///       --model "google/gemma-4-e4b" "Hello, what are you?"
///
/// Or set AGENCY_BASE_URL / AGENCY_MODEL env vars and just pass a prompt:
///   cargo run --example hello "Hello, what are you?"
use std::io::{self, Write};

use clap::Parser;
use futures::StreamExt;

use agency::{
    message::{ContentBlock, Message, Role},
    provider::{CompletionRequest, LlmProvider},
    providers::openai_compat::OpenAICompatProvider,
    stream::StreamEvent,
};

#[derive(Parser)]
#[command(about = "M1 example: single-turn streaming response from an OpenAI-compatible server")]
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

    prompt: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let provider = OpenAICompatProvider::new(&args.base_url, args.api_key);

    let request = CompletionRequest {
        model: args.model,
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: args.prompt }],
        }],
        system: Some("You are agency, an experimental LLM agent.".into()),
        temperature: Some(0.7),
        max_tokens: Some(512),
    };

    let mut stream = provider
        .stream(request)
        .await
        .expect("failed to start stream");

    while let Some(event) = stream.next().await {
        match event.expect("stream error") {
            StreamEvent::TextDelta(text) => {
                print!("{text}");
                io::stdout().flush().unwrap();
            }
            StreamEvent::MessageEnd { stop_reason } => {
                println!();
                eprintln!("[stop: {stop_reason:?}]");
                break;
            }
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
            } => {
                eprintln!("[usage: {input_tokens} in / {output_tokens} out]");
            }
            _ => {}
        }
    }
}
