//! `#[couture(prompt, editor)]` asks in the user's editor through `Backend::editor`.
#![cfg(feature = "interactive-editor")]

use core::cell::RefCell;
use std::io;

use clap::{ArgAction, ArgGroup, Parser, Subcommand};
use clap_couture::{
    Couture, CoutureParser as _, ParseError,
    interactive::{
        Backend, ConfirmPrompt, EditorPrompt, MultiSelectPrompt, PromptError, SecretPrompt,
        SelectPrompt, TextPrompt, secrecy::SecretString,
    },
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
}

#[derive(Parser, Couture, Debug)]
struct Unless {
    #[arg(long)]
    file: Option<String>,

    /// Notes.
    #[arg(long, required_unless_present = "file")]
    #[couture(prompt, editor)]
    notes: Option<String>,
}

#[derive(Parser, Couture, Debug)]
#[command(group(ArgGroup::new("source").required(true).args(["notes", "file"])))]
struct Grouped {
    #[arg(long)]
    file: Option<String>,

    /// Notes.
    #[arg(long)]
    #[couture(prompt, editor)]
    notes: Option<String>,
}

#[derive(Parser, Couture, Debug)]
#[command(group(ArgGroup::new("message").required(true).args(["body", "summary"])))]
struct TwoInGroup {
    /// Body.
    #[arg(long)]
    #[couture(prompt, editor)]
    body: Option<String>,

    /// Summary.
    #[arg(long)]
    #[couture(prompt, editor)]
    summary: Option<String>,
}

#[derive(Parser, Couture, Debug)]
#[command(name = "commit")]
struct Commit {
    /// Message.
    #[arg(long)]
    #[couture(prompt, editor(initial = "Summary\n\n# Why"))]
    message: String,

    /// Port.
    #[arg(long)]
    #[couture(prompt, editor(initial = port_template()))]
    port: Option<u16>,

    /// Trailer.
    #[arg(long, default_value = "Signed-off-by: a")]
    #[couture(prompt, editor(initial = "Signed-off-by: "))]
    trailer: String,
}

/// One `EditorPrompt`, owned.
#[derive(Debug, Default, PartialEq, Eq)]
struct Opened {
    error: Option<String>,
    extension: String,
    file_stem: String,
    initial: Option<String>,
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
            initial: prompt.initial.map(str::to_owned),
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

    fn multi_select(&self, _prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        Err(not_editor())
    }

    fn report(
        &self,
        error: &str,
        _styles: clap_couture::interactive::CommandStyles<'_>,
    ) -> Result<(), PromptError> {
        self.reported.borrow_mut().push(error.to_owned());
        Ok(())
    }

    fn secret(&self, _prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
        Err(not_editor())
    }

    fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(not_editor())
    }

    fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(not_editor())
    }
}

fn port_template() -> String {
    "8080".to_owned()
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
        initial: Some("Bug fixes.".to_owned()),
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
    assert_eq!(retry.and_then(|retry| retry.initial.as_deref()), Some("eighty"));
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
    assert!(matches!(err, Some(ParseError::Cancelled(_))), "{err:?}");
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
    let flags = Flags::couture_try_parse_from_with(&editor, ["flags"]);
    assert_eq!(flags.ok().map(|flags| flags.draft), Some(true));
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
    assert_eq!(retry.and_then(|retry| retry.initial.as_deref()), Some("eighty\nninety"));
    let error = retry.and_then(|retry| retry.error.clone()).unwrap_or_default();
    assert!(error.contains("invalid value 'eighty\u{2026}'"), "{error}");
    assert!(error.contains("invalid digit"), "{error}");
}

/// Whether each editor opened allowed an empty answer, oldest first.
fn optional(editor: &Editor) -> Vec<bool> {
    editor.opened.take().iter().map(|opened| opened.optional).collect()
}

#[rstest]
fn an_arg_required_unless_another_is_asked_as_required() {
    let editor = Editor::new([Some("Notes.")]);
    let unless = Unless::couture_try_parse_from_with(&editor, ["unless"]);
    assert_eq!(unless.ok().and_then(|unless| unless.notes).as_deref(), Some("Notes."));
    assert_eq!(optional(&editor), [false]);
}

#[rstest]
fn an_arg_required_unless_a_passed_one_is_optional() {
    let editor = Editor::new([None]);
    let unless = Unless::couture_try_parse_from_with(&editor, ["unless", "--file", "f"]);
    assert_eq!(unless.ok().map(|unless| unless.notes), Some(None));
    assert_eq!(optional(&editor), [true]);
}

#[rstest]
fn an_empty_answer_to_an_arg_required_by_a_rule_aborts() {
    let editor = Editor::new([None]);
    let unless = Unless::couture_try_parse_from_with(&editor, ["unless"]);
    assert!(unless.is_err_and(|err| err.to_string().contains("prompt cancelled")));
    assert_eq!(editor.reported.take(), ["Aborting due to an empty answer."]);
}

#[rstest]
fn the_only_prompted_arg_of_a_required_group_is_asked_as_required() {
    let editor = Editor::new([Some("Notes.")]);
    let grouped = Grouped::couture_try_parse_from_with(&editor, ["grouped"]);
    assert_eq!(grouped.ok().and_then(|grouped| grouped.notes).as_deref(), Some("Notes."));
    assert_eq!(optional(&editor), [false]);
}

#[rstest]
fn a_required_group_is_required_of_its_last_prompted_arg_only() {
    let editor = Editor::new([None, Some("Summary.")]);
    let message = TwoInGroup::couture_try_parse_from_with(&editor, ["message"]);
    assert_eq!(message.ok().and_then(|message| message.summary).as_deref(), Some("Summary."));
    // The body could be left for the summary; once it is, the summary can't.
    assert_eq!(optional(&editor), [true, false]);
}

#[rstest]
fn an_answered_group_member_satisfies_the_group() {
    let editor = Editor::new([Some("Body.")]);
    let message = TwoInGroup::couture_try_parse_from_with(&editor, ["message"]);
    assert_eq!(message.ok().and_then(|message| message.body).as_deref(), Some("Body."));
    // The group allows one member, so the summary isn't asked once the body is answered.
    assert_eq!(optional(&editor), [true]);
}

#[rstest]
fn the_prompter_asks_in_the_editor() {
    let editor = Editor::new([Some("Faster builds.")]);
    let answer = Release::prompter_with(&editor)
        .and_then(|prompter| prompter.editor("Notes?", "md", None).ok());
    assert_eq!(answer.as_deref(), Some("Faster builds."));
    assert_eq!(editor.opened.take(), [Opened {
        initial: None,
        error: None,
        extension: "md".to_owned(),
        file_stem: "release".to_owned(),
        instructions: "These lines are removed. Leave the file empty to cancel.".to_owned(),
        optional: false,
        question: "Notes?".to_owned(),
    }]);
}

#[rstest]
fn the_prompter_starts_the_file_with_its_initial_text() {
    let editor = Editor::new([Some("Faster builds.")]);
    let answer = Release::prompter_with(&editor)
        .and_then(|prompter| prompter.editor("Notes?", "md", Some("## Changes")).ok());
    assert_eq!(answer.as_deref(), Some("Faster builds."));
    let opened = editor.opened.take();
    assert_eq!(opened.first().and_then(|opened| opened.initial.as_deref()), Some("## Changes"));
}

#[rstest]
fn the_file_starts_with_the_initial_text() {
    let editor = Editor::new([Some("Fix it."), Some("80")]);
    let commit = Commit::couture_try_parse_from_with(&editor, ["commit", "--trailer", "t"]);
    assert_eq!(
        commit.ok().map(|commit| (commit.message, commit.port)),
        Some(("Fix it.".to_owned(), Some(80)))
    );
    let initials: Vec<_> = editor.opened.take().into_iter().map(|opened| opened.initial).collect();
    // A literal is kept as is; any other expression is evaluated.
    assert_eq!(initials, [Some("Summary\n\n# Why".to_owned()), Some("8080".to_owned())]);
}

#[rstest]
fn the_default_wins_over_the_initial_text() {
    let editor = Editor::new([Some("Signed-off-by: b")]);
    let commit =
        Commit::couture_try_parse_from_with(&editor, ["commit", "--message", "m", "--port", "1"]);
    assert_eq!(commit.ok().map(|commit| commit.trailer).as_deref(), Some("Signed-off-by: b"));
    let opened = editor.opened.take();
    assert_eq!(
        opened.first().and_then(|opened| opened.initial.as_deref()),
        Some("Signed-off-by: a")
    );
}

#[rstest]
fn a_rejected_answer_wins_over_the_initial_text() {
    let editor = Editor::new([Some("eighty"), Some("80")]);
    let commit = Commit::couture_try_parse_from_with(&editor, [
        "commit",
        "--message",
        "m",
        "--trailer",
        "t",
    ]);
    assert_eq!(commit.ok().and_then(|commit| commit.port), Some(80));
    let initials: Vec<_> = editor.opened.take().into_iter().map(|opened| opened.initial).collect();
    assert_eq!(initials, [Some("8080".to_owned()), Some("eighty".to_owned())]);
}

#[rstest]
fn an_empty_prompter_editor_answer_cancels() {
    let editor = Editor::new([None]);
    let answer =
        Release::prompter_with(&editor).map(|prompter| prompter.editor("Notes?", "md", None));
    assert!(matches!(answer, Some(Err(PromptError::Cancelled))));
    assert_eq!(editor.reported.take(), ["Aborting due to an empty answer."]);
}
