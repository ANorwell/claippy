use std::{env, error::Error, process};

fn main() {
    let cmd = CliCmd::parse_args(env::args()).unwrap_or_else(|err| {
        eprintln!("Error parsing arguments: {err}");
        process::exit(1);
    });
    println!("{:?}", cmd);
}

#[derive(Debug)]
pub enum CliCmd {
    AddToWorkspace { paths: Vec<String> },
    Query { query: String },
}

impl CliCmd {
    fn parse_args(mut args: impl Iterator<Item = String>) -> Result<CliCmd, Box<dyn Error>> {
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
