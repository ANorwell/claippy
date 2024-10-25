use std::{borrow::Cow, io::Cursor};

use colored::Colorize;
use rustyline::{
    completion::FilenameCompleter,
    error::ReadlineError,
    highlight::{Highlighter, MatchingBracketHighlighter},
    hint::HistoryHinter,
    history::DefaultHistory,
    validate::MatchingBracketValidator,
    Completer, ConditionalEventHandler, Editor, EventHandler, Helper, Hinter, KeyEvent, Validator,
};
use skim::prelude::*;

#[derive(Helper, Completer, Hinter, Validator)]
pub struct ReplHelper {
    #[rustyline(Completer)]
    completer: FilenameCompleter,
    highlighter: MatchingBracketHighlighter,
    #[rustyline(Validator)]
    validator: MatchingBracketValidator,
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    colored_prompt: String,
}

impl ReplHelper {
    pub fn new(prompt: &str) -> Self {
        Self {
            completer: FilenameCompleter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            validator: MatchingBracketValidator::new(),
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
        let options = SkimOptionsBuilder::default().multi(true).build().unwrap();

        let input = "aaaaa\nbbbb\nccc".to_string();

        // `SkimItemReader` is a helper to turn any `BufRead` into a stream of `SkimItem`
        // `SkimItem` was implemented for `AsRef<str>` by default
        let item_reader = SkimItemReader::default();
        let items = item_reader.of_bufread(Cursor::new(input));

        // `run_with` would read and show items from the stream
        let selected_items = Skim::run_with(&options, Some(items))
            .map(|out| out.selected_items)
            .unwrap_or_else(|| Vec::new());

        Some(rustyline::Cmd::Insert(
            1,
            selected_items
                .iter()
                .map(|i| i.output())
                .collect::<Vec<Cow<str>>>()
                .join(","),
        ))
    }
}
