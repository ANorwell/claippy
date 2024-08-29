use std::io::{self, Write};

use crate::model::MessageParts;
use crate::{
    db::Db,
    model::{Conversation, Result},
    query::Queryable,
    repl::make_readline,
};
use regex::Regex;
use rustyline::error::ReadlineError;
use termimad::MadSkin;

#[derive(Debug)]
pub enum CliCmd {
    NewConversation { conversation_id: String },
    AddWorkspaceContext { paths: Vec<String> },
    Repl,
    Query { query: String },
    Clear,
    ListWorkspaceContext,
    History
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
            "history" => Ok(CliCmd::History),
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
            Self::AddWorkspaceContext { paths } => {
                handle_add_workspace_contexts(db, paths)
            },
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
            },
            Self::History => todo!("not implemented")
        }
    }
}



fn handle_query(model: &impl Queryable, query: String, db: &Db) -> Result<CmdOutput>  {
    let skin = MadSkin::default();
    let mut conversation = db.read_current_conversation()?;
    conversation.add_user_message(query)?;
    let query_response = model.generate(conversation.as_messages().into())?;

    let mut full_content = String::new();
    let mut current_line = String::new();

    for chunk_result in query_response {
        let chunk = chunk_result?;
        for c in chunk.chars() {
            if c == '\n' {
                // Process and print the completed line
                print!("{}", skin.inline(&current_line));
                print!("\n");
                io::stdout().flush()?;
                full_content.push_str(&current_line);
                full_content.push('\n');
                current_line.clear();
            } else {
                current_line.push(c);
            }
        }
    }

    // If there's any remaining content in current_line, print it
    if !current_line.is_empty() {
        print!("{}", skin.inline(&current_line));
        io::stdout().flush()?;
        full_content.push_str(&current_line);
    }

    let parsed_message = parse_message_parts(full_content);
    print!("\n\n====Rendered message\n");
    print!("{}", format_message(&skin, &parsed_message));

    conversation.add_assistant_message(parsed_message);
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Done)
}

const CLAIPPY_ARTIFACT: &str = "ClaippyArtifact";

fn parse_message_parts(full_content: String) -> Vec<MessageParts> {
    let mut parts = Vec::new();
    let artifact_regex = Regex::new(&format!(r#"<{}\s+([^>]+)>([\s\S]?)</{}>"#, CLAIPPY_ARTIFACT, CLAIPPY_ARTIFACT)).unwrap();
    let mut last_end = 0;

    for cap in artifact_regex.captures_iter(&full_content) {
        let start = cap.get(0).unwrap().start();
        let end = cap.get(0).unwrap().end();

        // Add any text before the artifact as Markdown
        if start > last_end {
            parts.push(MessageParts::Markdown(full_content[last_end..start].to_string()));
        }

        // Parse attributes
        let attrs = cap.get(1).unwrap().as_str();
        let identifier_regex = Regex::new(r#"identifier="([^"]+)""#).unwrap();
        let language_regex = Regex::new(r#"language="([^"]+)""#).unwrap();

        let identifier = identifier_regex.captures(attrs)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let language = language_regex.captures(attrs)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        // Add the artifact
        parts.push(MessageParts::Artifact {
            identifier,
            language,
            content: cap.get(2).unwrap().as_str().to_string(),
        });

        last_end = end;
    }

    // Add any remaining text as Markdown
    if last_end < full_content.len() {
        parts.push(MessageParts::Markdown(full_content[last_end..].to_string()));
    }

    parts
}

fn format_message(skin: &MadSkin, full_message: &[MessageParts]) -> String {
    let mut formatted = String::new();
    for part in full_message {
        match part {
            MessageParts::Markdown(text) => {
                formatted.push_str(&skin.term_text(text).to_string());
            }
            MessageParts::Artifact { identifier, language, content } => {
                formatted.push_str(&format!("\nArtifact: {} (Language: {})\n", identifier,
    language.as_deref().unwrap_or("None")));
                formatted.push_str(&skin.term_text(content).to_string());
                formatted.push_str("\n");
            }
        }
    }

    formatted
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
                    let cmd = CliCmd::parse_args(cmd_str.split_whitespace().map(String::from))?;
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
            }
        }

        rl.save_history(&repl_history_path)?;
    }

    Ok(CmdOutput::Done)
}
