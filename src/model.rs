use std::error::Error;

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub type Result<T> = core::result::Result<T, Box<dyn Error>>;
pub type ResultIterator<T> = Result<Box<dyn Iterator<Item = T>>>;

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

impl <'a> Into<MessageRefs<'a>> for Vec<&'a Message> {
    fn into(self) -> MessageRefs<'a> {
        MessageRefs { messages: self }
    }
}

#[derive(Serialize, Deserialize)]
struct Artifact {
    pub text: String
}

impl Artifact {
    pub fn new(artifact: String) -> Artifact {
        Artifact { text: artifact }
    }

    pub fn extract_from_message(message: &Message) -> Option<Artifact> {
        let pattern = r"<Artifact>(.*?)</Artifact>";
        let re = Regex::new(pattern).unwrap();
        re.captures(&message.content)
            .and_then(|cap| cap.get(1))
            .map(|m| Artifact::new(m.as_str().into()))
    }
}

#[derive(Serialize, Deserialize)]
pub struct RichMessage {
    message: Message,
    artifact: Option<Artifact>
}


#[derive(Serialize, Deserialize)]
pub enum WorkspaceContext {
    File(String),
    Url(String)
}

#[derive(Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub context: Vec<WorkspaceContext>,
    pub messages: Vec<RichMessage>,
}

impl Conversation {
    pub fn create_id(descriptor: String) -> String {
        descriptor + "-" + &Utc::now().to_rfc3339()
    }
    pub fn empty(id: &str) -> Conversation {
        Conversation { id: id.to_owned(), context: Vec::new(), messages: Vec::new() }
    }

    pub fn as_message_refs(&self) -> Vec<&Message> {
        self.messages.iter().map(|rich| &rich.message).collect()
    }

}
