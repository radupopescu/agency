use std::io::{self, Write};

use futures::StreamExt;
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

use crate::{
    error::Result,
    message::{ContentBlock, Conversation, Message, Role},
    provider::{CompletionRequest, LlmProvider},
    stream::StreamEvent,
};

pub struct Repl {
    provider: Box<dyn LlmProvider>,
    model: String,
    system: Option<String>,
    conversation: Conversation,
}

impl Repl {
    pub fn new(provider: Box<dyn LlmProvider>, model: String, system: Option<String>) -> Self {
        Self {
            provider,
            model,
            system,
            conversation: Conversation::default(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("agency REPL — /help for commands, Ctrl-D to quit");

        let mut rl = Reedline::create();
        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic("you".to_owned()),
            DefaultPromptSegment::Empty,
        );

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
                println!("Conversation cleared.");
            }
            "quit" | "exit" => return true,
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
                println!("/clear          Clear conversation history");
                println!("/save [file]    Save conversation to JSON (default: conversation.json)");
                println!("/load <file>    Load conversation from JSON");
                println!("/quit           Exit the REPL");
            }
            other => eprintln!("Unknown command: /{other}  (try /help)"),
        }

        false
    }

    async fn send_and_stream(&mut self, input: &str) -> Result<()> {
        self.conversation.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: input.to_owned(),
            }],
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
