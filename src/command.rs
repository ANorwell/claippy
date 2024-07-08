use chrono::Utc;
use std::iter;

use crate::{db::Db, model::{Conversation, Message, MessageRefs, Result, ResultIterator}, query::Queryable};

#[derive(Debug)]
pub enum CliCmd {
    NewConversation { conversation_id: String },
    AddWorkspaceContext { paths: Vec<String> },
    Query { query: String },
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
    fn execute(self, model: impl Queryable, db: &Db) -> ResultIterator<Result<String>>;
}

impl Command for CliCmd {
    fn execute(self, model: impl Queryable, db: &Db) -> ResultIterator<Result<String>> {
        match self {
            Self::Query { query } => handle_query(model, query, db),
            Self::AddWorkspaceContext { paths } => {
                let mut conversation = db.read_current_conversation()?;
                let context_display = "Added context: ".to_owned() + &paths.join(", ");
                db.add_workspace_contexts(&mut conversation, paths)?;
                command_output(context_display)
            },
            Self::NewConversation { conversation_id } => {
                db.create_conversation(&conversation_id)?;
                command_output("Created conversation ".to_owned() + &conversation_id)
            }
        }
    }
}

fn command_output(message: String) -> ResultIterator<Result<String>> {
    let iter = iter::once(Ok(message));
    Ok(Box::new(iter))
}

fn handle_query(model: impl Queryable, query: String, db: &Db) -> ResultIterator<Result<String>> {
    // get conversation
    // convert conversation to ReqMessages (with new message added)
    // append new user message to Conversation

    let message = Message { role: "user".to_string(), content: query };
    model.generate(MessageRefs::new(vec!(&message)))

    //output

    // extract artifact
    // append new response to conversation
    // store in DB
}