use std::io::{self, Write};

use crate::model::MessageParts;
use crate::{
    db::Db,
    model::{Conversation, Result},
    query::Queryable,
    repl::make_readline,
};
use colored::Colorize;
use regex::Regex;
use rustyline::error::ReadlineError;
use termimad::crossterm::style::Stylize;
use termimad::{MadSkin, terminal_size};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

#[derive(Debug)]
pub enum CliCmd {
    NewConversation { conversation_id: String },
    AddWorkspaceContext { paths: Vec<String> },
    RemoveWorkspaceContext { paths: Vec<String> },
    Repl,
    Query { query: String },
    Clear,
    ListWorkspaceContext,
    History,
}

pub enum CmdOutput {
    Done,
    Message(String),
}

impl CliCmd {
    pub fn parse_args(mut args: impl Iterator<Item = String>) -> Result<CliCmd> {
        let cmd = args.next().unwrap_or("repl".to_owned());

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
            "remove" | "rm" => Ok(CliCmd::RemoveWorkspaceContext {
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
            Self::AddWorkspaceContext { paths } => handle_add_workspace_contexts(db, paths),
            Self::RemoveWorkspaceContext { paths } => handle_remove_workspace_contexts(db, paths),
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
            Self::History => {
                let conversation = db.read_current_conversation()?;
                let skin = MadSkin::default();
                for message in conversation.as_messages() {
                    println!("{}", message.role.blue().bold());
                    let parts = parse_message_parts(message.content);
                    println!("{}", format_message(&skin, &parts));
                }
                Ok(CmdOutput::Done)
            }
        }
    }
}

fn handle_query(model: &impl Queryable, query: String, db: &Db) -> Result<CmdOutput> {
    let skin = MadSkin::default();
    let mut conversation = db.read_current_conversation()?;
    conversation.add_user_message(query)?;
    let query_response = model.generate(conversation.as_messages().into())?;

    let mut full_content = String::new();
    let mut current_line = String::new();

    let mut line_count = 1;

    for chunk_result in query_response {
        let chunk = chunk_result?;
        for c in chunk.chars() {
            if c == '\n' {
                // Process and print the completed line
                print!("{}", skin.inline(&current_line));
                println!();
                line_count += 1;
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
        println!();
        line_count += 1;
        io::stdout().flush()?;
        full_content.push_str(&current_line);
    }

    erase_last_n_lines_simple(line_count);
    let parsed_message = parse_message_parts(full_content);
    println!("{}", format_message(&skin, &parsed_message));

    conversation.add_assistant_message(parsed_message);
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Done)
}

fn erase_last_n_lines_simple(n: usize) {
    // Move up N lines
    print!("\x1b[{}A", n);
    // Clear from cursor down
    print!("\x1b[J");
    // Flush stdout
    std::io::stdout().flush().unwrap();
}

const CLAIPPY_ARTIFACT: &str = "ClaippyArtifact";

fn parse_message_parts(full_content: String) -> Vec<MessageParts> {
    let mut parts = Vec::new();
    // Ideally use a real XML parser here instead
    let artifact_regex = Regex::new(&format!(
        r"(?ms)<{}\s*([^>]*?)>(.*?)</{}>",
        CLAIPPY_ARTIFACT, CLAIPPY_ARTIFACT
    ))
    .unwrap();
    let mut last_end = 0;

    for cap in artifact_regex.captures_iter(&full_content) {
        let start = cap.get(0).unwrap().start();
        let end = cap.get(0).unwrap().end();

        log::info!("Capture {start:?} {end:?} ");

        // Add any text before the artifact as Markdown
        if start > last_end {
            parts.push(MessageParts::Markdown(
                full_content[last_end..start].to_string(),
            ));
        }

        // Parse attributes
        let attrs = cap.get(1).unwrap().as_str();
        let identifier_regex = Regex::new(r#"identifier="([^"]+)""#).unwrap();
        let language_regex = Regex::new(r#"language="([^"]+)""#).unwrap();

        let identifier = identifier_regex
            .captures(attrs)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let language = language_regex
            .captures(attrs)
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
        log::info!("Emitted final markdown part")
    }

    parts
}

fn format_message(skin: &MadSkin, full_message: &[MessageParts]) -> String {
    let mut formatted = String::new();

    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let (term_width, _height) = terminal_size();

    for part in full_message {
        match part {
            MessageParts::Markdown(text) => {
                formatted.push_str(&skin.term_text(text).to_string());
            }
            MessageParts::Artifact {
                identifier,
                language,
                content,
            } => {
                let artifact_intro = format!(
                    "[Artifact: {} ({})]\n",
                    identifier,
                    language.as_deref().unwrap_or("None"));
                formatted.push_str(&format!("{}", artifact_intro.dim()));

                if let Some(lang) = language {
                    log::info!("Language: {}", lang);

                    if let Some(syntax) = ps.syntaxes().iter().find(|s| {
                        s.name.to_lowercase() == *lang || s.file_extensions.contains(lang)
                    }) {
                        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
                        let mut highlighted = String::new();

                        for line in LinesWithEndings::from(content) {
                            let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
                            let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
                            // Sets the line length to the term with, which allows the background formatting to
                            // extend to this length, which looks nicer.
                            highlighted.push_str(&format!("\x1b[{}X", term_width));
                            highlighted.push_str(&escaped);

                        }
                        highlighted.push_str("\x1b[0m"); // clear syntax, not handled by library

                        formatted.push_str(&highlighted);
                    } else {
                        log::warn!("No syntax found for language {}", lang);

                        // Fallback to regular formatting if syntax is not found
                        formatted.push_str(&content);
                    }
                } else {
                    // No language specified, use regular formatting
                    formatted.push_str(&content);
                }

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

fn handle_remove_workspace_contexts(db: &Db, paths: Vec<String>) -> Result<CmdOutput> {
    let mut conversation = db.read_current_conversation()?;
    let context_display = "Removed context:\n".to_owned() + &paths.join("\n");
    conversation.remove_workspace_contexts(&paths)?;
    db.write_conversation(&conversation)?;
    Ok(CmdOutput::Message(context_display))
}

fn handle_repl(model: &impl Queryable, db: &Db) -> Result<CmdOutput> {
    let prompt = format!("{}", Colorize::bold("claippy> ").cyan());
    let mut rl = make_readline(&prompt)?;

    let repl_history_path = db.path().join(".claippy-repl-history");

    if rl.load_history(&repl_history_path).is_err() {
        // No history, that's ok.
    }

    loop {
        let readline = rl.readline(&prompt);
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
                    if let Err(e) = handle_query(model, input.to_string(), db) {
                        println!("Query Error: {:?}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Read Error: {:?}", err);
            }
        }

        rl.save_history(&repl_history_path)?;
    }

    Ok(CmdOutput::Done)
}