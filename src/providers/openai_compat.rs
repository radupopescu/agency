use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    message::{ContentBlock, ImageData, Message, Role},
    provider::{CompletionRequest, EventStream, LlmProvider},
    stream::{StopReason, StreamEvent},
};

pub struct OpenAICompatProvider {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl OpenAICompatProvider {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key,
        }
    }
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<OaiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct OaiMessage {
    role: &'static str,
    content: OaiContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OaiContent {
    Text(String),
    Parts(Vec<OaiPart>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OaiPart {
    Text { text: String },
    ImageUrl { image_url: OaiImageUrl },
    File { file: OaiFile },
    InputAudio { input_audio: OaiAudio },
}

#[derive(Serialize)]
struct OaiImageUrl {
    url: String,
}

#[derive(Serialize)]
struct OaiFile {
    filename: String,
    file_data: String,
}

#[derive(Serialize)]
struct OaiAudio {
    data: String,
    format: String,
}

#[derive(Deserialize)]
struct ChatChunk {
    choices: Vec<ChunkChoice>,
    usage: Option<UsageData>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct UsageData {
    prompt_tokens: u32,
    completion_tokens: u32,
}

// ── Conversions ───────────────────────────────────────────────────────────────

fn role_str(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn text_content(msg: &Message) -> String {
    msg.content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn data_url(media_type: &str, data: &ImageData) -> String {
    match data {
        ImageData::Url(u) => u.clone(),
        ImageData::Base64(b) => format!("data:{media_type};base64,{b}"),
    }
}

/// Build the `content` field for a message. Plain-text messages serialise as a
/// bare string for maximum server compatibility; messages with images or files
/// use the OpenAI vision content-parts array.
fn oai_content(msg: &Message) -> OaiContent {
    let has_media = msg.content.iter().any(|b| {
        matches!(
            b,
            ContentBlock::Image { .. } | ContentBlock::File { .. } | ContentBlock::Audio { .. }
        )
    });

    if !has_media {
        return OaiContent::Text(text_content(msg));
    }

    let parts = msg
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(OaiPart::Text { text: text.clone() }),
            ContentBlock::Image { media_type, data } => Some(OaiPart::ImageUrl {
                image_url: OaiImageUrl {
                    url: data_url(media_type, data),
                },
            }),
            ContentBlock::File {
                name,
                media_type,
                data,
            } => Some(OaiPart::File {
                file: OaiFile {
                    filename: name.clone(),
                    file_data: data_url(media_type, data),
                },
            }),
            ContentBlock::Audio { format, data } => Some(OaiPart::InputAudio {
                input_audio: OaiAudio {
                    data: data.clone(),
                    format: format.clone(),
                },
            }),
            _ => None,
        })
        .collect();

    OaiContent::Parts(parts)
}

// ── SSE parsing ───────────────────────────────────────────────────────────────

fn parse_sse(response: reqwest::Response) -> impl Stream<Item = Result<StreamEvent>> + Send {
    async_stream::stream! {
        let mut byte_stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = byte_stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => { yield Err(Error::Provider(e.to_string())); return; }
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(nl) = buf.find('\n') {
                let line = buf[..nl].trim_end_matches('\r').to_owned();
                buf.drain(..=nl);

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d.to_owned(),
                    None => continue,
                };
                if data == "[DONE]" { return; }

                let chunk: ChatChunk = match serde_json::from_str(&data) {
                    Ok(c) => c,
                    Err(e) => { yield Err(Error::Json(e)); return; }
                };

                for choice in chunk.choices {
                    if let Some(text) = choice.delta.content
                        && !text.is_empty()
                    {
                        yield Ok(StreamEvent::TextDelta(text));
                    }
                    if let Some(reason) = choice.finish_reason {
                        let stop_reason = match reason.as_str() {
                            "stop"        => StopReason::EndTurn,
                            "tool_calls"  => StopReason::ToolUse,
                            "length"      => StopReason::MaxTokens,
                            other         => StopReason::Other(other.to_owned()),
                        };
                        yield Ok(StreamEvent::MessageEnd { stop_reason });
                    }
                }
                if let Some(u) = chunk.usage {
                    yield Ok(StreamEvent::Usage {
                        input_tokens: u.prompt_tokens,
                        output_tokens: u.completion_tokens,
                    });
                }
            }
        }
    }
}

// ── LlmProvider impl ──────────────────────────────────────────────────────────

#[async_trait]
impl LlmProvider for OpenAICompatProvider {
    async fn stream(&self, request: CompletionRequest) -> Result<EventStream> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut messages: Vec<OaiMessage> = Vec::new();
        if let Some(sys) = &request.system {
            messages.push(OaiMessage {
                role: "system",
                content: OaiContent::Text(sys.clone()),
            });
        }
        for msg in &request.messages {
            messages.push(OaiMessage {
                role: role_str(msg.role),
                content: oai_content(msg),
            });
        }

        let body = ChatRequest {
            model: request.model.clone(),
            messages,
            stream: true,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        let mut req = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let response = req
            .send()
            .await
            .map_err(|e| Error::Provider(e.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Provider(format!("HTTP {status}: {body}")));
        }

        Ok(Box::pin(parse_sse(response)))
    }

    fn name(&self) -> &str {
        "openai_compat"
    }
}
