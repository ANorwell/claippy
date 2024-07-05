use crate::model::{ReqMessage, ReqMessages, Model, Result, ResultIterator};

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
    fn execute(self, model: impl Model) -> ResultIterator<Result<String>>;
}

impl Command for CliCmd {
    fn execute(self, model: impl Model) -> ResultIterator<Result<String>> {
        match self {
            Self::Query { query } => handle_query(model, query),
            Self::AddToWorkspace { paths } => {
                let iter = paths.into_iter().map(|r| Ok(r));
                Ok(Box::new(iter))
            }
        }
    }
}

fn handle_query(model: impl Model, query: String) -> ResultIterator<Result<String>> {
    let message = ReqMessage { role: "user".to_string(), content: query };
    model.generate(ReqMessages::new(vec!(message)))
}