use std::{borrow::Cow, io::Cursor};

use colored::Colorize;
use ignore::WalkBuilder;
use rustyline::{
    completion::FilenameCompleter,
    error::ReadlineError,
    highlight::{Highlighter, MatchingBracketHighlighter},
    hint::HistoryHinter,
    history::DefaultHistory,
    Completer, ConditionalEventHandler, Editor, EventHandler, Helper, Hinter, KeyEvent, Validator,
};
use skim::prelude::*;

#[derive(Helper, Completer, Hinter, Validator)]
pub struct ReplHelper {
    #[rustyline(Completer)]
    completer: FilenameCompleter,
    highlighter: MatchingBracketHighlighter,
    #[rustyline(Validator)]
    validator: (),
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    colored_prompt: String,
}

impl ReplHelper {
    pub fn new(prompt: &str) -> Self {
        Self {
            completer: FilenameCompleter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            validator: (),
            hinter: HistoryHinter::new(),
            colored_prompt: prompt.to_owned(),
        }
    }
}

impl Highlighter for ReplHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        _prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Borrowed(&self.colored_prompt)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(hint.dimmed().to_string())
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

pub fn make_readline(prompt: &str) -> Result<Editor<ReplHelper, DefaultHistory>, ReadlineError> {
    let mut rl: Editor<ReplHelper, DefaultHistory> = Editor::new()?;
    let helper = ReplHelper::new(prompt);
    rl.set_helper(Some(helper));
    rl.bind_sequence(
        KeyEvent::ctrl('j'),
        EventHandler::Conditional(Box::new(SkimInserter)),
    );

    Ok(rl)
}

struct SkimInserter;


impl ConditionalEventHandler for SkimInserter {
    fn handle(
        &self,
        _evt: &rustyline::Event,
        _n: rustyline::RepeatCount,
        _positive: bool,
        _ctx: &rustyline::EventContext,
    ) -> Option<rustyline::Cmd> {
        // Get files from current directory, respecting gitignore
        let files = get_files_for_selection();
        if files.is_empty() {
            return Some(rustyline::Cmd::Insert(1, "No files found".to_string()));
        }

        // Create skim options
        let options = SkimOptionsBuilder::default()
            .height(Some("50%"))
            .multi(true)
            .preview(Some("")) // Empty string activates preview with default command
            .preview_window(Some("right:50%"))
            .build()
            .unwrap();

        // Create a source for skim
        let input = files.join("\n");
        let item_reader = SkimItemReader::default();
        let items = item_reader.of_bufread(Cursor::new(input));

        // Run skim and get selected items
        let selected_items = match Skim::run_with(&options, Some(items)) {
            Some(out) => {
                if out.is_abort {
                    return None;
                }
                out.selected_items
            }
            None => return None,
        };

        // Return selected file paths
        if !selected_items.is_empty() {
            Some(rustyline::Cmd::Insert(
                1,
                selected_items
                    .iter()
                    .map(|i| i.output())
                    .collect::<Vec<Cow<str>>>()
                    .join(" "),
            ))
        } else {
            None
        }
    }
}

/// Get files for selection, respecting gitignore rules and explicitly ignoring common directories
fn get_files_for_selection() -> Vec<String> {
    let mut files = Vec::new();

    // Common directories to explicitly ignore
    let common_ignores = [
        ".git", "node_modules", "target", "build", "dist",
        ".idea", ".vscode", "__pycache__", ".next", ".DS_Store"
    ];

   // Use WalkBuilder from the ignore crate to respect gitignore rules
   let mut walker = WalkBuilder::new(".");
   walker.hidden(false)        // Show hidden files (except those ignored)
        .git_ignore(true)      // Respect .gitignore
        .git_global(true)      // Respect global gitignore
        .git_exclude(true)     // Respect .git/info/exclude
        .require_git(false)    // Don't require a git repo to use git ignore rules
        .filter_entry(move |entry| {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            !common_ignores.iter().any(|ignore| &name == ignore)
        });

    for result in walker.build() {
        match result {
            Ok(entry) => {
                // Skip directories, only include files
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    if let Some(path) = entry.path().to_str() {
                        // Convert to relative path if it starts with ./
                        let path = if path.starts_with("./") {
                            &path[2..]
                        } else {
                            path
                        };
                        files.push(path.to_string());
                    }
                }
            }
            Err(_) => continue,
        }
    }

    files
}