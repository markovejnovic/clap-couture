//! `CoutureParser::prompter` asks your own questions through the parse's backend.
#![cfg(feature = "interactive")]

mod common;

use clap::{Parser, ValueEnum};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{PromptError, secrecy::ExposeSecret as _},
};
use common::{Asked, Kind, Reply, ScriptedBackend};
use rstest::rstest;

#[derive(Parser, Couture)]
#[couture(messages = { none = "(nothing)", selection_required = "Pick one" })]
struct Cli;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Region {
    Eu,
    #[value(hide = true)]
    Moon,
    Us,
}

fn asked(kind: Kind, question: &str) -> Asked {
    Asked {
        defaults: Vec::new(),
        error: None,
        hint: None,
        initial: None,
        kind,
        options: Vec::new(),
        question: question.to_owned(),
        selection_required: None,
    }
}

#[rstest]
fn there_is_no_prompter_without_a_terminal() {
    assert!(Cli::prompter_with(ScriptedBackend::unavailable()).is_none());
}

#[rstest]
fn confirm_asks_with_the_default() {
    let backend = ScriptedBackend::new([Reply::Confirm(true)]);
    let answer =
        Cli::prompter_with(&backend).and_then(|prompter| prompter.confirm("Go ahead?", true).ok());

    assert_eq!(answer, Some(true));
    assert_eq!(backend.asked(), [Asked {
        defaults: Vec::from(["true".to_owned()]),
        ..asked(Kind::Confirm, "Go ahead?")
    }]);
}

#[rstest]
fn text_returns_the_answer() {
    let backend = ScriptedBackend::new([Reply::Text(Some("web"))]);
    let answer = Cli::prompter_with(&backend).and_then(|prompter| prompter.text("App?").ok());

    assert_eq!(answer.as_deref(), Some("web"));
    assert_eq!(backend.asked(), [asked(Kind::Text, "App?")]);
}

#[rstest]
fn select_lists_the_visible_values() {
    let backend = ScriptedBackend::new([Reply::Select(Some("us"))]);
    let answer =
        Cli::prompter_with(&backend).and_then(|prompter| prompter.select::<Region>("Where?").ok());

    assert_eq!(answer, Some(Region::Us));
    assert_eq!(backend.asked(), [Asked {
        options: Vec::from(["eu".to_owned(), "us".to_owned()]),
        ..asked(Kind::Select, "Where?")
    }]);
}

#[rstest]
fn select_asks_again_after_an_unknown_value() {
    let backend = ScriptedBackend::new([Reply::Select(Some("asia")), Reply::Select(Some("eu"))]);
    let answer =
        Cli::prompter_with(&backend).and_then(|prompter| prompter.select::<Region>("Where?").ok());

    assert_eq!(answer, Some(Region::Eu));
    let errors: Vec<Option<String>> =
        backend.asked().into_iter().map(|asked| asked.error).collect();
    assert_eq!(errors, [None, Some("invalid value 'asia'".to_owned())]);
}

#[rstest]
fn multi_select_requires_one_with_the_commands_message() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&["us", "eu"])]);
    let answer = Cli::prompter_with(&backend)
        .and_then(|prompter| prompter.multi_select::<Region>("Where?").ok());

    assert_eq!(answer, Some(Vec::from([Region::Us, Region::Eu])));
    assert_eq!(backend.asked(), [Asked {
        options: Vec::from(["eu".to_owned(), "us".to_owned()]),
        selection_required: Some("Pick one".to_owned()),
        ..asked(Kind::MultiSelect, "Where?")
    }]);
}

#[rstest]
fn a_cancelled_prompt_is_cancelled() {
    let backend = ScriptedBackend::new([Reply::Cancel]);
    let answer = Cli::prompter_with(&backend).map(|prompter| prompter.text("App?"));

    assert!(matches!(answer, Some(Err(PromptError::Cancelled))));
}

#[rstest]
fn secret_returns_the_answer() {
    let backend = ScriptedBackend::new([Reply::Secret(Some("hunter2"))]);
    let answer = Cli::prompter_with(&backend)
        .and_then(|prompter| prompter.secret("Token?").ok())
        .map(|secret| secret.expose_secret().to_owned());

    assert_eq!(answer.as_deref(), Some("hunter2"));
    assert_eq!(backend.asked(), [asked(Kind::Secret, "Token?")]);
}

/// Run by [`prompter_unwrap_panics_without_a_terminal`] in a child with no terminal on stdin.
#[test]
#[ignore = "run in a child process by `prompter_unwrap_panics_without_a_terminal`"]
fn prompter_unwrap_in_child() {
    drop(Cli::prompter_unwrap());
}

#[rstest]
fn prompter_unwrap_panics_without_a_terminal() {
    let output = std::env::current_exe().and_then(|test_binary| {
        std::process::Command::new(test_binary)
            .args(["prompter_unwrap_in_child", "--exact", "--ignored"])
            .stdin(std::process::Stdio::null())
            .output()
    });
    let output = output.map(|output| (output.status.success(), output.stdout)).ok();
    assert!(
        output.as_ref().is_some_and(|(succeeded, stdout)| {
            !succeeded && String::from_utf8_lossy(stdout).contains("needs a terminal to ask on")
        }),
        "{output:?}"
    );
}
