use crate::{db::Db, model::{Message, MessageRefs, Result, ResultIterator}, query::Queryable};

#[derive(Debug)]
pub enum CliCmd {
    AddToWorkspace { paths: Vec<String> },
    Query { query: String },
}

impl CliCmd {
    pub fn parse_args(mut args: impl Iterator<Item = String>) -> Result<CliCmd> {
        args.next(); // ignore the script itself

        let cmd = args.next().ok_or("No command provided")?;

        let cmd = match cmd.as_str() {
            "add" | "a" => Ok(CliCmd::AddToWorkspace {
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
            Self::AddToWorkspace { paths } => {
                let iter = paths.into_iter().map(|r| Ok(r));
                Ok(Box::new(iter))
            }
        }
    }
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