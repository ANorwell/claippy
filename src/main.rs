use claippy::{
    command::{CliCmd, CmdOutput, Command},
    db::Db,
    query::Bedrock,
};
use std::{env, error::Error, process};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let cmd = CliCmd::parse_args(env::args()).unwrap_or_else(|err| {
        log::error!("Error parsing arguments: {err}");
        process::exit(1);
    });

    log::info!("Command: {:?}", cmd);

    let db = Db::create()?;

    let model = Bedrock::create("anthropic.claude-3-sonnet-20240229-v1:0".to_string())?;

    match cmd.execute(model, &db)? {
        CmdOutput::Message(msg) => print!("{}", msg),
        CmdOutput::Done => (), // do nothing
    }

    print!("\n");

    Ok(())
}
