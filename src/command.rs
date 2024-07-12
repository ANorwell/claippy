use crate::{
    db::Db,
    model::{Artifact, Conversation, Result},
    query::Queryable,
};

#[derive(Debug)]
pub enum CliCmd {
    NewConversation { conversation_id: String },
    AddWorkspaceContext { paths: Vec<String> },
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
        args.next(); // ignore the script itself

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
            Self::AddWorkspaceContext { paths } => handle_add_workspace_contexts(db, paths),
            Self::NewConversation { conversation_id } => {
                db.create_conversation(&conversation_id)?;
                Ok(CmdOutput::Message("Created conversation ".to_owned() + &conversation_id))
            }
            Self::Clear => {
                let mut conversation = db.read_current_conversation()?;
                conversation.clear()?;
                db.write_conversation(&conversation)?;
                Ok(CmdOutput::Message("Cleared conversation ".to_owned() + &conversation.id))
            }
            Self::ListWorkspaceContext => {
                let conversation = db.read_current_conversation()?;
                let contexts = conversation.context;
                let context_display = "Current context:\n".to_owned() +
                    &contexts.into_iter().map(|c| c.to_string()).collect::<Vec<String>>().join("\n");
                Ok(CmdOutput::Message(context_display))
            }
        }
    }
}

fn handle_query(model: impl Queryable, query: String, db: &Db) -> Result<CmdOutput> {
    let mut conversation = db.read_current_conversation()?;
    conversation.add_user_message(query);
    let query_response = model.generate(conversation.as_message_refs().into())?;

    let mut full_message = String::new();

    for chunk_result in query_response {
        let chunk = chunk_result?;
        print!("{}", chunk);
        full_message += &chunk;
    }

    let artifact = Artifact::extract_from_message(&full_message);

    conversation.add_assistant_message(full_message, artifact);
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Done)
}

fn handle_add_workspace_contexts(db: &Db, paths: Vec<String>) -> Result<CmdOutput> {
    let mut conversation = db.read_current_conversation()?;
    let context_display = "Added context: ".to_owned() + &paths.join(", ");
    conversation.add_workspace_contexts(paths)?;
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Message(context_display))
}
