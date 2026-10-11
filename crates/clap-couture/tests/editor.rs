//! `#[couture(prompt, editor)]` asks in the user's editor through `Backend::editor`.
#![cfg(feature = "interactive-editor")]

use core::cell::RefCell;
use std::io;

use clap::{ArgAction, Parser, Subcommand, error::ErrorKind};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{Backend, ConfirmPrompt, EditorPrompt, PromptError, SelectPrompt, TextPrompt},
};
use rstest::rstest;

#[derive(Parser, Couture, Debug)]
#[command(name = "release")]
struct Release {
    /// Release notes.
    #[arg(long, default_value = "Bug fixes.")]
    #[couture(prompt, editor(extension = "md"))]
    notes: String,

    #[arg(long)]
    #[couture(prompt = "Which port?", editor)]
    port: Option<u16>,

    /// Who signs off.
    #[arg(long)]
    #[couture(prompt, editor)]
    reviewer: String,
}

#[derive(Parser, Couture, Debug)]
#[couture(messages = { editor_instructions = "Leer lassen zum Abbrechen." })]
struct German {
    /// Notizen.
    #[arg(long)]
    #[couture(prompt, editor)]
    notes: String,
}

#[derive(Parser, Couture, Debug)]
#[command(name = "tool")]
struct Tool {
    #[command(subcommand)]
    command: ToolCommand,
}

#[derive(Subcommand, Couture, Debug)]
enum ToolCommand {
    Ship {
        /// Release notes.
        #[arg(long)]
        #[couture(prompt, editor)]
        notes: String,
    },
}

#[derive(Parser, Couture, Debug)]
struct Flags {
    /// Draft?
    #[arg(long, action = ArgAction::Set)]
    #[couture(prompt, editor)]
    draft: bool,

    /// How loud.
    #[arg(short, action = ArgAction::Count)]
    #[couture(prompt, editor)]
    verbose: u8,
}

/// One `EditorPrompt`, owned.
#[derive(Debug, Default, PartialEq, Eq)]
struct Opened {
    default: Option<String>,
    error: Option<String>,
    extension: String,
    file_stem: String,
    instructions: String,
    optional: bool,
    question: String,
}

/// Answers every editor prompt from a script, oldest reply first, and fails any other prompt.
struct Editor {
    opened: RefCell<Vec<Opened>>,
    replies: RefCell<Vec<Option<&'static str>>>,
    reported: RefCell<Vec<String>>,
}

impl Editor {
    fn new(replies: impl IntoIterator<Item = Option<&'static str>>) -> Self {
        let mut replies: Vec<_> = replies.into_iter().collect();
        replies.reverse();
        Self {
            opened: RefCell::new(Vec::new()),
            replies: RefCell::new(replies),
            reported: RefCell::new(Vec::new()),
        }
    }
}

impl Backend for Editor {
    fn confirm(&self, _prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Err(not_editor())
    }

    fn editor(&self, prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
        self.opened.borrow_mut().push(Opened {
            default: prompt.default.map(str::to_owned),
            error: prompt.error.map(str::to_owned),
            extension: prompt.extension.to_owned(),
            file_stem: prompt.file_stem.to_owned(),
            instructions: prompt.instructions.to_owned(),
            optional: prompt.optional,
            question: prompt.question.to_owned(),
        });
        let reply = self.replies.borrow_mut().pop();
        reply
            .map(|answer| answer.map(str::to_owned))
            .ok_or_else(|| PromptError::Io(io::Error::other("the script ran out of replies")))
    }

    fn is_available(&self) -> bool {
        true
    }

    fn report(
        &self,
        error: &str,
        _styles: clap_couture::interactive::CommandStyles<'_>,
    ) -> Result<(), PromptError> {
        self.reported.borrow_mut().push(error.to_owned());
        Ok(())
    }

    fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(not_editor())
    }

    fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(not_editor())
    }
}

fn not_editor() -> PromptError {
    PromptError::Io(io::Error::other("only editor prompts should be asked"))
}

#[rstest]
fn a_missing_arg_opens_the_editor_with_its_default() {
    let editor = Editor::new([Some("Faster builds.\n\nAnd more.")]);
    let release = Release::couture_try_parse_from_with(&editor, [
        "release",
        "--port",
        "1",
        "--reviewer",
        "a",
    ]);
    assert_eq!(
        release.ok().map(|release| release.notes).as_deref(),
        Some("Faster builds.\n\nAnd more.")
    );
    assert_eq!(editor.opened.take(), [Opened {
        default: Some("Bug fixes.".to_owned()),
        error: None,
        extension: "md".to_owned(),
        file_stem: "release-notes".to_owned(),
        // Emptied, it would fall back to the default, so an empty file cancels instead.
        instructions: "These lines are removed. Leave the file empty to cancel.".to_owned(),
        optional: false,
        question: "Release notes".to_owned(),
    }]);
}

#[rstest]
fn a_rejected_answer_reopens_with_its_text_and_error() {
    let editor = Editor::new([Some("eighty"), Some("80")]);
    let release = Release::couture_try_parse_from_with(&editor, [
        "release",
        "--notes",
        "n",
        "--reviewer",
        "a",
    ]);
    assert_eq!(release.ok().and_then(|release| release.port), Some(80));
    let opened = editor.opened.take();
    let retry = opened.get(1);
    assert_eq!(retry.and_then(|retry| retry.default.as_deref()), Some("eighty"));
    let error = retry.and_then(|retry| retry.error.clone()).unwrap_or_default();
    assert!(error.contains("invalid value 'eighty'"), "{error}");
    assert_eq!(editor.reported.take(), [error]);
}

#[rstest]
fn an_empty_optional_answer_leaves_the_arg_unset() {
    let editor = Editor::new([None]);
    let release = Release::couture_try_parse_from_with(&editor, [
        "release",
        "--notes",
        "n",
        "--reviewer",
        "a",
    ]);
    assert_eq!(release.map(|release| release.port).ok(), Some(None));
    let opened = editor.opened.take();
    assert_eq!(opened.first().map(|opened| opened.optional), Some(true));
    assert_eq!(
        opened.first().map(|opened| opened.instructions.as_str()),
        Some("These lines are removed. Leave the file empty to leave it unset.")
    );
}

#[rstest]
fn an_empty_required_answer_aborts() {
    let editor = Editor::new([None]);
    let release =
        Release::couture_try_parse_from_with(&editor, ["release", "--notes", "n", "--port", "1"]);
    let err = release.err();
    assert_eq!(err.as_ref().map(clap::Error::kind), Some(ErrorKind::Io));
    assert!(err.is_some_and(|err| err.to_string().contains("prompt cancelled")));
    assert_eq!(editor.reported.take(), ["Aborting due to an empty answer."]);
}

#[rstest]
fn a_passed_arg_is_not_asked() {
    let editor = Editor::new([]);
    let release = Release::couture_try_parse_from_with(&editor, [
        "release",
        "--notes",
        "n",
        "--port",
        "1",
        "--reviewer",
        "a",
    ]);
    assert_eq!(release.ok().map(|release| release.reviewer).as_deref(), Some("a"));
    assert_eq!(editor.opened.take(), []);
}

#[rstest]
fn messages_localize_the_instructions() {
    let editor = Editor::new([Some("Hallo")]);
    let german = German::couture_try_parse_from_with(&editor, ["german"]);
    assert_eq!(german.ok().map(|german| german.notes).as_deref(), Some("Hallo"));
    let opened = editor.opened.take();
    assert_eq!(
        opened.first().map(|opened| opened.instructions.as_str()),
        Some("Leer lassen zum Abbrechen.")
    );
}

#[rstest]
fn the_file_is_named_after_the_subcommand_path() {
    let editor = Editor::new([Some("Shipped.")]);
    let tool = Tool::couture_try_parse_from_with(&editor, ["tool", "ship"]);
    assert!(tool.is_ok(), "{tool:?}");
    let opened = editor.opened.take();
    assert_eq!(opened.first().map(|opened| opened.file_stem.as_str()), Some("tool-ship-notes"));
}

#[rstest]
fn a_bool_that_takes_a_value_opens_the_editor() {
    let editor = Editor::new([Some("true")]);
    let flags = Flags::couture_try_parse_from_with(&editor, ["flags", "-v"]);
    assert_eq!(flags.ok().map(|flags| flags.draft), Some(true));
}

#[rstest]
fn a_counted_flag_is_not_edited() {
    let editor = Editor::new([]);
    let flags = Flags::couture_try_parse_from_with(&editor, ["flags", "--draft", "true"]);
    let err = flags.err().map(|err| err.to_string()).unwrap_or_default();
    // It takes no value, so it is asked as a plain prompt, which this backend refuses.
    assert!(err.contains("only editor prompts should be asked"), "{err}");
    assert_eq!(editor.opened.take(), []);
}

#[rstest]
fn an_empty_answer_to_an_arg_with_a_default_aborts() {
    let editor = Editor::new([None]);
    let release = Release::couture_try_parse_from_with(&editor, [
        "release",
        "--port",
        "1",
        "--reviewer",
        "a",
    ]);
    assert!(release.is_err_and(|err| err.to_string().contains("prompt cancelled")));
    assert_eq!(editor.reported.take(), ["Aborting due to an empty answer."]);
}

#[rstest]
fn a_rejected_multi_line_answer_keeps_the_reason() {
    let editor = Editor::new([Some("eighty\nninety"), Some("80")]);
    let release = Release::couture_try_parse_from_with(&editor, [
        "release",
        "--notes",
        "n",
        "--reviewer",
        "a",
    ]);
    assert_eq!(release.ok().and_then(|release| release.port), Some(80));
    let opened = editor.opened.take();
    let retry = opened.get(1);
    assert_eq!(retry.and_then(|retry| retry.default.as_deref()), Some("eighty\nninety"));
    let error = retry.and_then(|retry| retry.error.clone()).unwrap_or_default();
    assert!(error.contains("invalid value 'eighty\u{2026}'"), "{error}");
    assert!(error.contains("invalid digit"), "{error}");
}
