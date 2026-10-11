//! A secret's env var is used without asking, and neither `--help` nor a confirmation shows its
//! value. This test sets a process-wide variable, so it lives alone in its own binary.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "the env var answers, so only a confirmation is scripted")]
mod common;

use clap::Parser;
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::secrecy::{ExposeSecret as _, SecretString},
};
use common::{Reply, ScriptedBackend};
use rstest::rstest;

#[derive(Parser, Couture)]
struct Cli {
    /// API token.
    #[arg(long, env = "CLAP_COUTURE_TEST_TOKEN")]
    #[couture(prompt, secret)]
    token: SecretString,
}

/// A computed question escapes the derive's check, so the runtime keeps the secret out of it.
#[derive(Parser, Couture)]
struct Confirmed {
    #[arg(long, env = "CLAP_COUTURE_TEST_TOKEN")]
    #[couture(prompt, secret)]
    token: SecretString,

    #[arg(long)]
    #[couture(confirm = String::from("Use token {token}?"))]
    yes: bool,
}

#[rstest]
fn the_env_var_is_used_and_hidden_from_help() {
    // SAFETY: this binary holds a single test, so no other thread reads the environment.
    unsafe {
        std::env::set_var("CLAP_COUTURE_TEST_TOKEN", "hunter2");
    }
    let backend = ScriptedBackend::new([]);
    let token = Cli::couture_try_parse_from_with(&backend, ["app"])
        .map(|cli| cli.token.expose_secret().to_owned());
    assert_eq!(token.ok().as_deref(), Some("hunter2"));
    assert_eq!(backend.asked(), []);

    let help = Cli::couture_command().render_help().to_string();
    assert!(help.contains("CLAP_COUTURE_TEST_TOKEN"), "{help}");
    assert!(!help.contains("hunter2"), "{help}");

    let backend = ScriptedBackend::new([Reply::Confirm(true)]);
    let confirmed = Confirmed::couture_try_parse_from_with(&backend, ["app"]);
    assert_eq!(confirmed.map(|confirmed| confirmed.yes).ok(), Some(true));
    let questions: Vec<String> = backend.asked().into_iter().map(|asked| asked.question).collect();
    assert_eq!(questions, ["Use token ********?"]);
}
