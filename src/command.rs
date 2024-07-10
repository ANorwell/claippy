use chrono::Utc;
use std::iter;

use crate::{db::Db, model::{Conversation, Message, MessageRefs, Result, ResultIterator}, query::Queryable};

#[derive(Debug)]
pub enum CliCmd {
    NewConversation { conversation_id: String },
    AddWorkspaceContext { paths: Vec<String> },
    Query { query: String },
}

pub enum CmdOutput {
    Done,
    Message(String)
}

impl CliCmd {
    pub fn parse_args(mut args: impl Iterator<Item = String>) -> Result<CliCmd> {
        args.next(); // ignore the script itself

        let cmd = args.next().ok_or("No command provided")?;

        let cmd = match cmd.as_str() {
            "new" | "n" => {
                let conversation_id = Conversation::create_id(args.collect::<Vec<String>>().join("-"));
                 Ok(CliCmd::NewConversation { conversation_id  })
            },
            "add" | "a" => Ok(CliCmd::AddWorkspaceContext {
                paths: args.collect(),
            }),
            "query" | "q" => Ok(CliCmd::Query {
                query: args.collect::<Vec<String>>().join(" "),
            }),
            other => Err(format!("Unknown command: {other}")),
        }?;

        Ok(cmd)
    }
}

pub trait Command {
    fn execute(self, model: impl Queryable, db: &Db) -> Result<CmdOutput>;
}

impl Command for CliCmd {
    fn execute(self, model: impl Queryable, db: &Db) -> Result<CmdOutput> {
        match self {
            Self::Query { query } => handle_query(model, query, db),
            Self::AddWorkspaceContext { paths } => {
                let mut conversation = db.read_current_conversation()?;
                let context_display = "Added context: ".to_owned() + &paths.join(", ");
                db.add_workspace_contexts(&mut conversation, paths)?;
                Ok(CmdOutput::Message(context_display))
            },
            Self::NewConversation { conversation_id } => {
                db.create_conversation(&conversation_id)?;
                Ok(CmdOutput::Message("Created conversation ".to_owned() + &conversation_id))
            }
        }
    }
}

fn handle_query(model: impl Queryable, query: String, db: &Db) -> Result<CmdOutput> {
    let mut conversation = db.read_current_conversation()?;
    conversation.add_user_message(query);
    let query_response =model.generate(conversation.as_message_refs().into())?;

    let mut full_message = String::new();

    for chunk_result in query_response {
        let chunk = chunk_result?;
        print!("{}", chunk);
        full_message += &chunk;
    }

    // extract artifact from the message if there is one

    conversation.add_assistant_message(full_message);
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Done)
}