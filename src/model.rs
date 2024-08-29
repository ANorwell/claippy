use std::{
    collections::HashSet,
    error::Error,
    fmt::{Display, Formatter, Write},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};

pub type Result<T> = core::result::Result<T, Box<dyn Error>>;
pub type ResultIterator<'a, T> = Result<Box<dyn Iterator<Item = T> + 'a>>;

const USER_ROLE: &str = "user";
const ASSISTANT_ROLE: &str = "assistant";

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct Messages {
    pub messages: Vec<Message>,
}

impl Messages {
    pub fn new(messages: Vec<Message>) -> Messages {
        Messages { messages }
    }
}

impl From<Vec<Message>> for Messages {
    fn from(messages: Vec<Message>) -> Self {
        Messages { messages }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageParts {
    Markdown(String),
    Artifact {
        identifier: String,
        language: Option<String>,
        content: String,
    }
}

#[derive(Serialize, Deserialize)]
pub struct RichMessage {
    role: String,
    parts: Vec<MessageParts>
}

impl RichMessage {
    pub fn as_message(&self) -> Message {
        let content = self.parts
            .iter()
            .map(|part| match part {
                MessageParts::Markdown(text) => text.clone(),
                MessageParts::Artifact { identifier, language, content } => {
                    let lang_attr = language
                        .as_ref()
                        .map(|lang| format!(" language=\"{}\"", lang))
                        .unwrap_or_default();
                    format!("<ClaippyArtifact identifier=\"{}\"{}>\n{}\n</ClaippyArtifact>",
                        identifier, lang_attr, content)
                }
            })
            .collect::<Vec<String>>()
            .join("\n\n");

        Message {
            role: self.role.clone(),
            content,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkspaceContext {
    File(String),
    Url(String),
}

impl WorkspaceContext {
    pub fn retrieve(&self) -> Result<String> {
        let (src, contents) = match self {
            WorkspaceContext::File(path) => (path, std::fs::read_to_string(path)?),
            WorkspaceContext::Url(url) => (url, reqwest::blocking::get(url)?.text()?),
        };

        let mut wrapped_contents = String::with_capacity(src.len() + contents.len() + 40);
        write!(
            wrapped_contents,
            r#"<ClaippyContext src="{src}">{contents}</ClaippyContext>"#
        )?;
        Ok(wrapped_contents)
    }
}

impl From<String> for WorkspaceContext {
    fn from(raw: String) -> Self {
        if raw.starts_with("http://") || raw.starts_with("https://") {
            WorkspaceContext::Url(raw)
        } else {
            WorkspaceContext::File(raw)
        }
    }
}

impl Display for WorkspaceContext {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            WorkspaceContext::File(path) => f.write_str(path),
            WorkspaceContext::Url(url) => f.write_str(url),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,

    // The API only allows us to send one user message at a time, so we track which context is seen and unseen.
    // When new context is added, it'll get prepended to the next message.
    pub unseen_context: HashSet<WorkspaceContext>,
    pub seen_context: HashSet<WorkspaceContext>,

    pub messages: Vec<RichMessage>,
}

impl Conversation {
    pub fn create_id(descriptor: String) -> String {
        descriptor + "-" + &Utc::now().to_rfc3339()
    }
    pub fn empty(id: &str) -> Conversation {
        Conversation {
            id: id.to_owned(),
            unseen_context: HashSet::new(),
            seen_context: HashSet::new(),
            messages: Vec::new(),
        }
    }

    pub fn add_workspace_contexts(&mut self, raw_contexts: Vec<String>) -> Result<()> {
        for raw_context in raw_contexts {
            let context: WorkspaceContext = raw_context.into();
            self.unseen_context.insert(context);
        }

        Ok(())
    }

    // Clears the conversation, but not the context (all context will become unseen)
    pub fn clear(&mut self) -> Result<()> {
        self.messages.clear();
        self.unseen_context.extend(self.seen_context.drain());
        Ok(())
    }

    pub fn add_user_message(&mut self, message: String) -> Result<()> {
        let mut user_message = String::with_capacity(message.len());
        for context in self.unseen_context.drain() {
            user_message += &context.retrieve()?;
            user_message += "\n";
            self.seen_context.insert(context);
        }

        user_message += &message;

        self.messages.push(self.user_message(user_message));
        Ok(())
    }

    pub fn add_assistant_message(&mut self, message: Vec<MessageParts>) {
        self.messages.push(RichMessage { role: ASSISTANT_ROLE.to_owned(), parts: message });
    }


    pub fn as_messages(&self) -> Vec<Message> {
        self.messages.iter().map(|rich| rich.as_message()).collect()
    }

    fn user_message(&self, content: String) -> RichMessage {
        RichMessage {
            role: USER_ROLE.to_owned(),
            parts: vec!(MessageParts::Markdown(content)),
        }
    }
}
