/// Send a local image and stream the model's description.
///
/// Run with:
///   cargo run --example vision -- --model "google/gemma-4-e4b" path/to/image.png
///
/// Or via env vars (model must support vision):
///   AGENCY_MODEL="google/gemma-4-e4b" cargo run --example vision -- path/to/image.png
///
/// Add `--prompt` to override the default question.
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use clap::Parser;
use futures::StreamExt;

use agency::{
    error::{Error, Result},
    message::{ContentBlock, ImageData, Message, Role},
    provider::{CompletionRequest, LlmProvider},
    providers::openai_compat::OpenAICompatProvider,
    stream::StreamEvent,
};

#[derive(Parser)]
#[command(about = "M4 example: send a local image and stream the description")]
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

    #[arg(long, default_value = "Describe this image in detail.")]
    prompt: String,

    image: PathBuf,
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

    let bytes = std::fs::read(&args.image)?;
    let media_type = guess_media_type(&args.image).ok_or_else(|| {
        Error::Config(format!(
            "unrecognised image extension: {}",
            args.image.display()
        ))
    })?;

    let image_block = ContentBlock::Image {
        media_type: media_type.into(),
        data: ImageData::Base64(BASE64.encode(&bytes)),
    };

    let provider = OpenAICompatProvider::new(&args.base_url, args.api_key);

    let request = CompletionRequest {
        model: args.model,
        messages: vec![Message {
            role: Role::User,
            content: vec![image_block, ContentBlock::Text { text: args.prompt }],
        }],
        system: None,
        temperature: None,
        max_tokens: Some(512),
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

fn guess_media_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}
