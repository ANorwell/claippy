use std::{env, error::Error, process};
use claippy::{command::{CliCmd, Command}, model::Bedrock};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let cmd = CliCmd::parse_args(env::args()).unwrap_or_else(|err| {
        log::info!("Error parsing arguments: {err}");
        process::exit(1);
    });


    log::info!("Command: {:?}", cmd);


    let model = Bedrock::create("anthropic.claude-3-sonnet-20240229-v1:0".to_string())?;
    for output in cmd.execute(model)? {
        print!("{}", output?);
    }

    print!("\n");

    Ok(())
}
