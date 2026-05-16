use std::borrow::Cow;
use std::io::{self, Write};
use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crossterm::style::Color;
use futures::StreamExt;
use nu_ansi_term::Style;
use reedline::{
    Highlighter, Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal, StyledText,
};

use crate::{
    error::Result,
    message::{ContentBlock, Conversation, ImageData, Message, Role},
    provider::{CompletionRequest, LlmProvider},
    stream::StreamEvent,
};

/// Leaves typed input in the terminal's default color instead of reedline's
/// default command highlighting.
struct PlainHighlighter;

impl Highlighter for PlainHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();
        styled.push((Style::default(), line.to_owned()));
        styled
    }
}

/// A minimal `>>` prompt in the terminal's default color.
struct MonoPrompt;

impl Prompt for MonoPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(">> ")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(":: ")
    }
    fn render_prompt_history_search_indicator(&self, _search: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Borrowed("?? ")
    }
    fn get_prompt_color(&self) -> Color {
        Color::Reset
    }
    fn get_prompt_multiline_color(&self) -> nu_ansi_term::Color {
        nu_ansi_term::Color::Default
    }
    fn get_indicator_color(&self) -> Color {
        Color::Reset
    }
    fn get_prompt_right_color(&self) -> Color {
        Color::Reset
    }
}

pub struct Repl {
    provider: Box<dyn LlmProvider>,
    base_url: String,
    model: String,
    system: Option<String>,
    conversation: Conversation,
    pending_attachments: Vec<ContentBlock>,
}

impl Repl {
    pub fn new(
        provider: Box<dyn LlmProvider>,
        base_url: String,
        model: String,
        system: Option<String>,
    ) -> Self {
        Self {
            provider,
            base_url,
            model,
            system,
            conversation: Conversation::default(),
            pending_attachments: Vec::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("agency REPL");
        println!("  provider : {}", self.base_url);
        println!("  model    : {}", self.model);
        println!("/help for commands, Ctrl-D to quit");

        let mut rl = Reedline::create().with_highlighter(Box::new(PlainHighlighter));
        let prompt = MonoPrompt;

        loop {
            let signal = tokio::task::block_in_place(|| rl.read_line(&prompt));

            match signal {
                Ok(Signal::Success(line)) => {
                    let input = line.trim().to_owned();
                    if input.is_empty() {
                        continue;
                    }
                    if input.starts_with('/') {
                        if self.handle_command(&input) {
                            break;
                        }
                    } else if let Err(e) = self.send_and_stream(&input).await {
                        eprintln!("error: {e}");
                    }
                }
                Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => break,
                Err(e) => {
                    eprintln!("Input error: {e}");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Returns true if the REPL should exit.
    fn handle_command(&mut self, input: &str) -> bool {
        let (cmd, arg) = match input[1..].split_once(' ') {
            Some((c, a)) => (c, a.trim()),
            None => (&input[1..], ""),
        };

        match cmd {
            "clear" => {
                self.conversation.messages.clear();
                self.pending_attachments.clear();
                println!("Conversation cleared.");
            }
            "quit" | "exit" => return true,
            "attach" => {
                if arg.is_empty() {
                    eprintln!("Usage: /attach <path>");
                } else {
                    match read_attachment(arg) {
                        Ok(block) => {
                            let label = attachment_label(&block);
                            self.pending_attachments.push(block);
                            println!("Attached {label}; will be sent with your next message.");
                        }
                        Err(e) => eprintln!("Attach failed: {e}"),
                    }
                }
            }
            "save" => {
                let path = if arg.is_empty() {
                    "conversation.json"
                } else {
                    arg
                };
                match serde_json::to_string_pretty(&self.conversation) {
                    Ok(json) => match std::fs::write(path, json) {
                        Ok(()) => println!("Saved to {path}"),
                        Err(e) => eprintln!("Save failed: {e}"),
                    },
                    Err(e) => eprintln!("Serialize error: {e}"),
                }
            }
            "load" => {
                if arg.is_empty() {
                    eprintln!("Usage: /load <file>");
                } else {
                    match std::fs::read_to_string(arg) {
                        Ok(json) => match serde_json::from_str(&json) {
                            Ok(conv) => {
                                self.conversation = conv;
                                println!(
                                    "Loaded {} messages from {arg}",
                                    self.conversation.messages.len()
                                );
                            }
                            Err(e) => eprintln!("Parse error: {e}"),
                        },
                        Err(e) => eprintln!("Read error: {e}"),
                    }
                }
            }
            "help" => {
                println!(
                    "/attach <path>  Attach an image, audio file, or document to the next message"
                );
                println!("/clear          Clear conversation history and pending attachments");
                println!("/save [file]    Save conversation to JSON (default: conversation.json)");
                println!("/load <file>    Load conversation from JSON");
                println!("/quit           Exit the REPL");
            }
            other => eprintln!("Unknown command: /{other}  (try /help)"),
        }

        false
    }

    async fn send_and_stream(&mut self, input: &str) -> Result<()> {
        let mut content = std::mem::take(&mut self.pending_attachments);
        content.push(ContentBlock::Text {
            text: input.to_owned(),
        });
        self.conversation.push(Message {
            role: Role::User,
            content,
        });

        let request = CompletionRequest {
            model: self.model.clone(),
            messages: self.conversation.messages.clone(),
            system: self.system.clone(),
            temperature: None,
            max_tokens: None,
        };

        let mut stream = self.provider.stream(request).await?;
        let mut assistant_text = String::new();

        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::TextDelta(text) => {
                    print!("{text}");
                    io::stdout().flush()?;
                    assistant_text.push_str(&text);
                }
                StreamEvent::MessageEnd { .. } => break,
                _ => {}
            }
        }
        println!();

        if !assistant_text.is_empty() {
            self.conversation.push(Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: assistant_text,
                }],
            });
        }

        Ok(())
    }
}

/// Read a local file, base64-encode it, and wrap it as an `Image`, `Audio`, or
/// `File` content block based on the extension.
fn read_attachment(path: &str) -> Result<ContentBlock> {
    let bytes = std::fs::read(path)?;
    let b64 = BASE64.encode(&bytes);

    if let Some(format) = guess_audio_format(path) {
        return Ok(ContentBlock::Audio {
            format: format.to_owned(),
            data: b64,
        });
    }

    let media_type = guess_media_type(path);
    let data = ImageData::Base64(b64);
    if media_type.starts_with("image/") {
        Ok(ContentBlock::Image { media_type, data })
    } else {
        let name = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_owned();
        Ok(ContentBlock::File {
            name,
            media_type,
            data,
        })
    }
}

fn guess_media_type(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
    .to_owned()
}

/// OpenAI's `input_audio` part only accepts `"wav"` or `"mp3"`. Other audio
/// extensions fall through to the generic file path.
fn guess_audio_format(path: &str) -> Option<&'static str> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    match ext.as_str() {
        "wav" => Some("wav"),
        "mp3" => Some("mp3"),
        _ => None,
    }
}

fn attachment_label(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Image { media_type, .. } => format!("image ({media_type})"),
        ContentBlock::Audio { format, .. } => format!("audio ({format})"),
        ContentBlock::File {
            name, media_type, ..
        } => format!("{name} ({media_type})"),
        _ => "attachment".to_owned(),
    }
}
