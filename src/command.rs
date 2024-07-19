use crate::{
    db::Db,
    model::{Artifact, Conversation, Result},
    query::Queryable,
    repl::make_readline,
};
use rustyline::error::ReadlineError;

#[derive(Debug)]
pub enum CliCmd {
    NewConversation { conversation_id: String },
    AddWorkspaceContext { paths: Vec<String> },
    Repl,
    Query { query: String },
    Clear,
    ListWorkspaceContext,
}

pub enum CmdOutput {
    Done,
    Message(String),
}

impl CliCmd {
    pub fn parse_args(mut args: impl Iterator<Item = String>) -> Result<CliCmd> {
        let cmd = args.next().ok_or("No command provided")?;

        let cmd = match cmd.as_str() {
            "query" | "q" => Ok(CliCmd::Query {
                query: args.collect::<Vec<String>>().join(" "),
            }),
            "new" | "n" => {
                let conversation_id =
                    Conversation::create_id(args.collect::<Vec<String>>().join("-"));
                Ok(CliCmd::NewConversation { conversation_id })
            }
            "add" | "a" => Ok(CliCmd::AddWorkspaceContext {
                paths: args.collect(),
            }),
            "clear" => Ok(CliCmd::Clear),
            "ls" => Ok(CliCmd::ListWorkspaceContext),
            "repl" => Ok(CliCmd::Repl),
            other => Err(format!("Unknown command: {other}")),
        }?;

        Ok(cmd)
    }
}

pub trait Command {
    fn execute(self, model: &impl Queryable, db: &Db) -> Result<CmdOutput>;
}

impl Command for CliCmd {
    fn execute(self, model: &impl Queryable, db: &Db) -> Result<CmdOutput> {
        match self {
            Self::Query { query } => handle_query(model, query, db),
            Self::Repl => handle_repl(model, db),
            Self::AddWorkspaceContext { paths } => handle_add_workspace_contexts(db, paths),
            Self::NewConversation { conversation_id } => {
                db.create_conversation(&conversation_id)?;
                Ok(CmdOutput::Message(
                    "Created conversation ".to_owned() + &conversation_id,
                ))
            }
            Self::Clear => {
                let mut conversation = db.read_current_conversation()?;
                conversation.clear()?;
                db.write_conversation(&conversation)?;
                Ok(CmdOutput::Message(
                    "Cleared conversation ".to_owned() + &conversation.id,
                ))
            }
            Self::ListWorkspaceContext => {
                let conversation = db.read_current_conversation()?;
                let contexts = conversation
                    .seen_context
                    .into_iter()
                    .chain(conversation.unseen_context);
                let context_display = "Current context:\n".to_owned()
                    + &contexts
                        .map(|c| c.to_string())
                        .collect::<Vec<String>>()
                        .join("\n");
                Ok(CmdOutput::Message(context_display))
            }
        }
    }
}

fn handle_query(model: &impl Queryable, query: String, db: &Db) -> Result<CmdOutput> {
    let mut conversation = db.read_current_conversation()?;
    conversation.add_user_message(query)?;
    let query_response = model.generate(conversation.as_message_refs().into())?;

    let mut full_message = String::new();

    for chunk_result in query_response {
        let chunk = chunk_result?;
        print!("{}", chunk);
        full_message += &chunk;
    }

    println!("\n"); // and flush

    let artifact = Artifact::extract_from_message(&full_message);

    conversation.add_assistant_message(full_message, artifact);
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Done)
}

fn handle_add_workspace_contexts(db: &Db, paths: Vec<String>) -> Result<CmdOutput> {
    let mut conversation = db.read_current_conversation()?;
    let context_display = "Added context:\n".to_owned() + &paths.join("\n");
    conversation.add_workspace_contexts(paths)?;
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Message(context_display))
}

fn handle_repl(model: &impl Queryable, db: &Db) -> Result<CmdOutput> {
    let prompt = "> ";
    let mut rl = make_readline(prompt)?;

    let repl_history_path = db.path().join(".claippy-repl-history");

    if rl.load_history(&repl_history_path).is_err() {
        // No history, that's ok.
    }

    loop {
        let readline = rl.readline(prompt);
        match readline {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                let input = line.trim_start();

                if let Some(cmd_str) = input.strip_prefix('!') {
                    let cmd =
                        CliCmd::parse_args(cmd_str.split_whitespace().map(String::from))?;
                    match cmd.execute(model, db)? {
                        CmdOutput::Done => continue,
                        CmdOutput::Message(msg) => println!("{}", msg),
                    }
                } else {
                    handle_query(model, input.to_string(), db)?;
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }

        rl.save_history(&repl_history_path)?;
    }

    Ok(CmdOutput::Done)
}
