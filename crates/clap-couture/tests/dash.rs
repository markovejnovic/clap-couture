//! `#[couture(prompt(dash))]` asks for an arg when `-` is its value, and only then.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only text and secret replies are scripted here")]
mod common;

use clap::{Parser, error::ErrorKind};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::secrecy::{ExposeSecret, SecretString},
};
use common::{Kind, Reply, ScriptedBackend};
use rstest::rstest;

#[derive(Parser, Couture, Debug)]
struct Commit {
    #[arg(long)]
    #[couture(prompt(dash, question = "Who wrote it?"))]
    author: Option<String>,

    /// Commit message.
    #[arg(short, long)]
    #[couture(prompt(dash))]
    message: Option<String>,

    /// Path to commit.
    #[couture(prompt(dash))]
    path: Option<String>,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Parser, Couture)]
struct Login {
    #[arg(long)]
    #[couture(prompt(dash), secret)]
    token: Option<SecretString>,
}

/// The questions `backend` asked, oldest first.
fn questions(backend: &ScriptedBackend) -> Vec<String> {
    backend.asked().into_iter().map(|asked| asked.question).collect()
}

#[rstest]
#[case::by_long_flag(&["commit", "--message", "-"])]
#[case::by_long_flag_with_equals(&["commit", "--message=-"])]
#[case::by_short_flag(&["commit", "-m", "-"])]
#[case::by_attached_short_flag(&["commit", "-m-"])]
fn a_dash_asks_for_the_arg(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Text(Some("Fix login"))]);
    let commit = Commit::couture_try_parse_from_with(&backend, argv);
    assert_eq!(commit.map(|commit| commit.message).ok().flatten().as_deref(), Some("Fix login"));
    assert_eq!(questions(&backend), ["Commit message"]);
}

#[rstest]
#[case::apart(&["commit", "-vm", "-"])]
#[case::attached(&["commit", "-vm-"])]
#[case::attached_with_equals(&["commit", "-vm=-"])]
#[case::before_the_flag(&["commit", "-m", "-", "-v"])]
fn a_dash_keeps_the_other_args(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Text(Some("Fix login"))]);
    let commit = Commit::couture_try_parse_from_with(&backend, argv);
    let parsed = commit.map(|commit| (commit.message, commit.verbose)).ok();
    assert_eq!(parsed, Some((Some("Fix login".to_owned()), true)));
}

#[rstest]
#[case::alone(&["commit", "-"])]
#[case::after_a_double_dash(&["commit", "--", "-"])]
fn a_dash_asks_for_a_positional(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Text(Some("src"))]);
    let commit = Commit::couture_try_parse_from_with(&backend, argv);
    assert_eq!(commit.map(|commit| commit.path).ok().flatten().as_deref(), Some("src"));
    assert_eq!(questions(&backend), ["Path to commit"]);
}

#[test]
fn a_dash_asks_its_own_question() {
    let backend = ScriptedBackend::new([Reply::Text(Some("ada"))]);
    let commit = Commit::couture_try_parse_from_with(&backend, ["commit", "--author", "-"]);
    assert_eq!(commit.map(|commit| commit.author).ok().flatten().as_deref(), Some("ada"));
    assert_eq!(questions(&backend), ["Who wrote it?"]);
}

#[test]
fn every_dash_is_asked_in_declaration_order() {
    let backend = ScriptedBackend::new([Reply::Text(Some("ada")), Reply::Text(Some("Fix"))]);
    let argv = ["commit", "-m", "-", "--author", "-"];
    let commit = Commit::couture_try_parse_from_with(&backend, argv);
    let parsed = commit.map(|commit| (commit.author, commit.message)).ok();
    assert_eq!(parsed, Some((Some("ada".to_owned()), Some("Fix".to_owned()))));
    assert_eq!(questions(&backend), ["Who wrote it?", "Commit message"]);
}

#[rstest]
#[case::left_out(&["commit"], None)]
#[case::passed(&["commit", "-m", "typed"], Some("typed"))]
fn without_a_dash_nothing_is_asked(#[case] argv: &[&str], #[case] message: Option<&str>) {
    let backend = ScriptedBackend::new([]);
    let commit = Commit::couture_try_parse_from_with(&backend, argv);
    assert_eq!(commit.map(|commit| commit.message).ok(), Some(message.map(str::to_owned)));
    assert_eq!(questions(&backend), Vec::<String>::new());
}

#[test]
fn help_exits_before_asking() {
    let backend = ScriptedBackend::new([]);
    let commit = Commit::couture_try_parse_from_with(&backend, ["commit", "-m", "-", "--help"]);
    assert_eq!(commit.map(drop).map_err(|err| err.kind()), Err(ErrorKind::DisplayHelp));
}

#[test]
fn without_a_terminal_a_dash_is_an_error() {
    let backend = ScriptedBackend::unavailable();
    let commit = Commit::couture_try_parse_from_with(&backend, ["commit", "-m", "-"]);
    let err = commit.err().map(|err| (err.kind(), err.to_string()));
    let message =
        "error: --message <MESSAGE> got `-` to be asked for, but there is no terminal to ask on";
    assert!(
        err.as_ref().is_some_and(|(kind, rendered)| {
            *kind == ErrorKind::ValueValidation && rendered.starts_with(message)
        }),
        "{err:?}"
    );
}

#[test]
fn without_a_terminal_and_without_a_dash_nothing_fails() {
    let backend = ScriptedBackend::unavailable();
    let commit = Commit::couture_try_parse_from_with(&backend, ["commit", "-m", "typed"]);
    assert_eq!(commit.map(|commit| commit.message).ok().flatten().as_deref(), Some("typed"));
}

#[test]
fn a_dash_asks_for_a_secret_the_command_line_may_not_pass() {
    let backend = ScriptedBackend::new([Reply::Secret(Some("hunter2"))]);
    let login = Login::couture_try_parse_from_with(&backend, ["login", "--token", "-"]);
    let token = login.ok().and_then(|login| login.token);
    assert_eq!(token.as_ref().map(ExposeSecret::expose_secret), Some("hunter2"));
    let asked: Vec<_> = backend.asked().into_iter().map(|asked| asked.kind).collect();
    assert_eq!(asked, [Kind::Secret]);
}

#[test]
fn a_secret_passed_on_the_command_line_is_still_refused() {
    let backend = ScriptedBackend::new([]);
    let login = Login::couture_try_parse_from_with(&backend, ["login", "--token", "hunter2"]);
    assert_eq!(login.map(drop).map_err(|err| err.kind()), Err(ErrorKind::ValueValidation));
}
