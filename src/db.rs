use std::{fs, path::PathBuf};

use crate::model::{Conversation, Result, WorkspaceContext};

/// Stores and retrieves conversations by conversation ID.
/// Right now this uses/overwrites files, but it could use e.g. sqlite internally
pub struct Db {
    path: PathBuf,
}

impl Db {
    const CURRENT_PATH: &'static str = "current";

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

    pub fn create_conversation(&self, conversation_id: &str) -> Result<()> {
        let conversation = Conversation::empty(conversation_id);
        self.write_conversation(&conversation)?;
        std::os::unix::fs::symlink(
            self.path.join(&conversation_id),
            self.path.join(Self::CURRENT_PATH),
        )?;
        Ok(())
    }

    // Reads a conversation. If no conversation exists, creates and returns an empty one.
    pub fn read_conversation(&self, conversation_id: &str) -> Result<Conversation> {
        let file_path = self.path.join(&conversation_id);

        if !file_path.exists() {
            let conversation_to_create = if conversation_id.eq(Self::CURRENT_PATH) {
                &Conversation::create_id("untitled-conversation".to_owned())
            } else {
                conversation_id
            };
            self.create_conversation(conversation_to_create)?
        }

        let bytes = fs::read(file_path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn read_current_conversation(&self) -> Result<Conversation> {
        self.read_conversation(Self::CURRENT_PATH)
    }
}
