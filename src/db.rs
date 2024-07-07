use std::{fs, path::PathBuf};

use crate::model::{Conversation, Result};

/// Stores and retrieves conversations by conversation ID.
///


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




