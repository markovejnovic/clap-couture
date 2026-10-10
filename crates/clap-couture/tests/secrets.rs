//! `#[couture(prompt, secret)]` asks masked, keeps the answer out of clap, and refuses the command
//! line.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only tests/interactive.rs scripts confirm, select and text replies")]
mod common;

use clap::{Parser, error::ErrorKind};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{
        NonInteractive,
        secrecy::{ExposeSecret as _, SecretString},
    },
};
use common::ScriptedBackend;
use rstest::rstest;

#[derive(Parser, Couture)]
struct Login {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret)]
    token: SecretString,
}

#[derive(Parser, Couture)]
struct OptionalLogin {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret)]
    token: Option<SecretString>,
}

#[derive(Parser, Couture)]
struct ScriptedLogin {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret(allow_argv))]
    token: SecretString,
}

#[rstest]
fn the_derive_records_the_secret_and_its_argv_policy() {
    let allow_argv = |tree: &'static clap_couture::interactive::PromptedArgTree| {
        tree.prompted_args()
            .first()
            .and_then(|arg| arg.attr.secret.as_ref())
            .map(|secret| secret.allow_argv)
    };
    assert_eq!(allow_argv(&Login::PROMPTED_ARGS), Some(false));
    assert_eq!(allow_argv(&OptionalLogin::PROMPTED_ARGS), Some(false));
    assert_eq!(allow_argv(&ScriptedLogin::PROMPTED_ARGS), Some(true));
}

#[rstest]
fn the_command_line_is_refused_before_anything_is_asked() {
    let backend = ScriptedBackend::new([]);
    let login = Login::couture_try_parse_from_with(&backend, ["app", "--token", "x"]);
    assert_eq!(login.map(drop).map_err(|err| err.kind()).err(), Some(ErrorKind::ValueValidation));
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn the_command_line_is_refused_without_a_terminal() {
    let login = Login::couture_try_parse_from_with(&NonInteractive, ["app", "--token", "x"]);
    assert_eq!(login.map(drop).map_err(|err| err.kind()).err(), Some(ErrorKind::ValueValidation));
}

#[rstest]
fn an_optional_secret_is_refused_on_the_command_line() {
    let login =
        OptionalLogin::couture_try_parse_from_with(&NonInteractive, ["app", "--token", "x"]);
    assert_eq!(login.map(drop).map_err(|err| err.kind()).err(), Some(ErrorKind::ValueValidation));
}

#[rstest]
fn the_refusal_names_the_arg() {
    let login = Login::couture_try_parse_from_with(&NonInteractive, ["app", "--token", "x"]);
    let message = login.map(drop).map_err(|err| err.to_string()).err().unwrap_or_default();
    assert!(message.contains("--token <TOKEN> is a secret"), "{message}");
}

#[rstest]
fn couture_command_alone_refuses_the_command_line() {
    let matches = Login::couture_command().try_get_matches_from(["app", "--token", "x"]);
    assert_eq!(matches.map(drop).map_err(|err| err.kind()).err(), Some(ErrorKind::ValueValidation));
}

#[rstest]
fn allow_argv_takes_the_command_line() {
    let login =
        ScriptedLogin::couture_try_parse_from_with(&NonInteractive, ["app", "--token", "x"]);
    let token = login.map(|login| login.token.expose_secret().to_owned());
    assert_eq!(token.ok().as_deref(), Some("x"));
}
