//! `#[couture(prompt(dash))]` asks for an arg when `-` is its value, and only then.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only text and secret replies are scripted here")]
mod common;

use clap::{Parser, ValueEnum, error::ErrorKind};
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

#[derive(Parser, Couture, Debug)]
struct Serve {
    /// Log level.
    #[arg(long, value_enum)]
    #[couture(prompt(dash))]
    level: Option<Level>,

    /// Port to listen on.
    #[arg(long)]
    #[couture(prompt(dash))]
    port: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Level {
    Debug,
    Info,
}

#[derive(Parser, Couture)]
struct Login {
    #[arg(long)]
    #[couture(prompt(dash), secret)]
    token: Option<SecretString>,
}

#[derive(Parser, Couture)]
struct Greet {
    /// Name to greet.
    #[arg(long)]
    #[couture(prompt(dash))]
    #[expect(clippy::option_option, reason = "clap's arg whose value may be left out")]
    name: Option<Option<String>>,
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

#[rstest]
#[case::left_out(&["greet"], "None")]
#[case::without_a_value(&["greet", "--name"], "Some(None)")]
#[case::passed(&["greet", "--name", "ada"], "Some(Some(\"ada\"))")]
fn an_optional_value_without_a_dash_parses_as_clap_parses_it(
    #[case] argv: &[&str],
    #[case] name: &str,
) {
    let backend = ScriptedBackend::new([]);
    let greet = Greet::couture_try_parse_from_with(&backend, argv);
    let parsed = greet.map(|greet| format!("{:?}", greet.name)).map_err(|err| err.to_string());
    assert_eq!(parsed.as_deref(), Ok(name));
    assert_eq!(questions(&backend), Vec::<String>::new());
}

#[test]
fn a_dash_asks_for_an_optional_value() {
    let backend = ScriptedBackend::new([Reply::Text(Some("ada"))]);
    let greet = Greet::couture_try_parse_from_with(&backend, ["greet", "--name", "-"]);
    let parsed = greet.map(|greet| greet.name).map_err(|err| err.to_string());
    assert_eq!(parsed, Ok(Some(Some("ada".to_owned()))));
    assert_eq!(questions(&backend), ["Name to greet"]);
}

#[test]
fn help_exits_before_asking() {
    let backend = ScriptedBackend::new([]);
    let commit = Commit::couture_try_parse_from_with(&backend, ["commit", "-m", "-", "--help"]);
    assert_eq!(commit.map(drop).map_err(|err| err.kind()), Err(Some(ErrorKind::DisplayHelp)));
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
            *kind == Some(ErrorKind::ValueValidation) && rendered.starts_with(message)
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
    assert_eq!(login.map(drop).map_err(|err| err.kind()), Err(Some(ErrorKind::ValueValidation)));
}

#[test]
fn a_rejected_answer_is_asked_again_with_claps_message() {
    let backend = ScriptedBackend::new([Reply::Text(Some("abc")), Reply::Text(Some("8080"))]);
    let serve = Serve::couture_try_parse_from_with(&backend, ["serve", "--port", "-"]);
    assert_eq!(serve.map(|serve| serve.port).ok().flatten(), Some(8080));
    let errors: Vec<_> = backend.asked().into_iter().map(|asked| asked.error).collect();
    assert_eq!(errors, [
        None,
        Some("invalid value 'abc' for '--port <PORT>': invalid digit found in string".to_owned())
    ]);
}

#[test]
fn a_dash_offers_the_possible_values() {
    let backend = ScriptedBackend::new([Reply::Select(Some("info"))]);
    let serve = Serve::couture_try_parse_from_with(&backend, ["serve", "--level", "-"]);
    assert_eq!(serve.map(|serve| serve.level).ok().flatten(), Some(Level::Info));
    let asked: Vec<_> =
        backend.asked().into_iter().map(|asked| (asked.kind, asked.options)).collect();
    assert_eq!(asked, [(Kind::Select, vec!["debug".to_owned(), "info".to_owned()])]);
}

#[test]
fn a_value_other_than_dash_fails_as_clap_fails_it() {
    #[derive(Parser, Debug)]
    #[command(name = "serve")]
    struct PlainServe {
        #[arg(long)]
        port: Option<u16>,
    }

    let backend = ScriptedBackend::new([]);
    let argv = ["serve", "--port", "abc"];
    let serve = Serve::couture_try_parse_from_with(&backend, argv);
    let plain = PlainServe::try_parse_from(argv);
    assert_eq!(
        serve.err().map(|err| (err.kind(), err.to_string())),
        plain.err().map(|err| (Some(err.kind()), err.to_string()))
    );
}
