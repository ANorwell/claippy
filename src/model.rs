use std::{collections::HashSet, error::Error, fmt::Write};

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub type Result<T> = core::result::Result<T, Box<dyn Error>>;
pub type ResultIterator<T> = Result<Box<dyn Iterator<Item = T>>>;

const USER_ROLE: &str = "user";
const ASSISTANT_ROLE: &str = "assistant";

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct MessageRefs<'a> {
    pub messages: Vec<&'a Message>,
}

impl MessageRefs<'_> {
    pub fn new(messages: Vec<&Message>) -> MessageRefs {
        MessageRefs { messages }
    }
}

impl<'a> Into<MessageRefs<'a>> for Vec<&'a Message> {
    fn into(self) -> MessageRefs<'a> {
        MessageRefs { messages: self }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub language: Option<String>,
    pub text: String,
}

impl Artifact {
    pub fn extract_from_message(message: &str) -> Option<Artifact> {
        let pattern = r"<ClaippyArtifact.*?</ClaippyArtifact>";
        let re = Regex::new(pattern).unwrap();
        re.captures(&message)
            .and_then(|cap| cap.get(0))
            .and_then(|m| Artifact::parse_artifact_xml(m.as_str()))
    }

    fn parse_artifact_xml(xml: &str) -> Option<Artifact> {
        let doc = roxmltree::Document::parse(xml).ok()?;
        let elem = doc.descendants().find(|n| n.tag_name().name() == "ClippyArtifact")?;

        let id = elem.attribute("identifier")?;
        let language = elem.attribute("language");
        let text = elem.text()?;

        Some(Artifact { id: id.into(), language: language.map(|s| s.into()), text: text.into() })
    }
}

#[derive(Serialize, Deserialize)]
pub struct RichMessage {
    message: Message,
    artifact: Option<Artifact>,
}

impl RichMessage {
    pub fn new(message: Message, artifact: Option<Artifact>) -> RichMessage {
        RichMessage { message, artifact }
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
        write!(wrapped_contents, r#"<ClaippyContext src="{src}">{contents}</ClaippyContext>"#)?;
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

impl ToString for WorkspaceContext {
    fn to_string(&self) -> String {
        match self {
            WorkspaceContext::File(path) => path.to_owned(),
            WorkspaceContext::Url(url) => url.to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub context: HashSet<WorkspaceContext>,
    pub messages: Vec<RichMessage>,
}

impl Conversation {
    pub fn create_id(descriptor: String) -> String {
        descriptor + "-" + &Utc::now().to_rfc3339()
    }
    pub fn empty(id: &str) -> Conversation {
        Conversation {
            id: id.to_owned(),
            context: HashSet::new(),
            messages: Vec::new(),
        }
    }

    pub fn add_workspace_contexts(&mut self, raw_contexts: Vec<String>) -> Result<()> {
        for raw_context in raw_contexts {
            let context: WorkspaceContext = raw_context.into();
            self.messages.push(self.user_message(context.retrieve()?));
            self.context.insert(context);
        }

        Ok(())
    }

    // Clears the conversation, but not the context
    pub fn clear(&mut self) -> Result<()> {
        self.messages.clear();
        for context in &self.context {
            self.messages.push(self.user_message(context.retrieve()?));
        }
        Ok(())
    }

    pub fn add_user_message(&mut self, message: String) {
        self.messages.push(self.user_message(message));
    }

    pub fn add_assistant_message(&mut self, message: String, artifact: Option<Artifact>) {
        self.messages.push(RichMessage::new(Message {
            role: ASSISTANT_ROLE.to_owned(),
            content: message,
        }, artifact));
    }

    pub fn as_message_refs(&self) -> Vec<&Message> {
        self.messages.iter().map(|rich| &rich.message).collect()
    }

    fn user_message(&self, content: String) -> RichMessage {
        RichMessage::new(Message {
            role: USER_ROLE.to_owned(),
            content,
        }, None)
    }
}
