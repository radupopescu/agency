use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// An image or file payload: either a URL the provider fetches itself, or
/// base64-encoded bytes sent inline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageData {
    Url(String),
    Base64(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        media_type: String,
        data: ImageData,
    },
    File {
        name: String,
        media_type: String,
        data: ImageData,
    },
    /// Audio attachment. OpenAI-compat `input_audio` is base64-only, with
    /// `format` restricted to `"wav"` or `"mp3"`.
    Audio {
        format: String,
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Conversation {
    pub messages: Vec<Message>,
}

impl Conversation {
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }
}
