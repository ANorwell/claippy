use regex::Regex;
use std::{fs, path::PathBuf};
use serde::{Deserialize, Serialize};

use crate::model::{Message, Result};

/// Stores and retrieves conversations by conversation ID.
///


#[derive(Serialize, Deserialize)]
struct Artifact {
    text: String
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
pub struct Conversation {
    id: String,
    messages: Vec<RichMessage>
}

impl Conversation {
    pub fn as_message_refs(&self) -> Vec<&Message> {
        self.messages.iter().map(|rich| &rich.message).collect()
    }
}

pub struct Db {
    path: PathBuf,
}

impl Db {
    pub fn create() -> Result<Db> {
        let mut path: PathBuf = std::env::current_dir()?;
        loop {
            if path.join(".git").is_dir() {
                path = path.join(".claippy");
                if !path.is_dir() {
                    fs::create_dir_all(&path)?;
                }
                return Ok(Db { path });
            }
            if !path.pop() {
                return Err("No .git directory found in any parent directory".into());
            }
        }
    }

    pub fn write_conversation(&self, conversation: &Conversation) -> Result<()> {
        let file_path = self.path.join(&conversation.id);
        fs::write(file_path, serde_json::to_string_pretty(conversation)?)?;
        Ok(())
    }

    pub fn read_conversation(&self, conversation_id: &str) -> Result<Conversation> {
        let file_path = self.path.join(conversation_id);
        let bytes = fs::read(file_path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}




