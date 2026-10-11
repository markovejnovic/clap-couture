//! A secret's env var is used without asking, and `--help` doesn't show its value. This test sets a
//! process-wide variable, so it lives alone in its own binary.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "the env var answers, so no reply is ever scripted")]
mod common;

use clap::Parser;
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::secrecy::{ExposeSecret as _, SecretString},
};
use common::ScriptedBackend;
use rstest::rstest;

#[derive(Parser, Couture)]
struct Cli {
    /// API token.
    #[arg(long, env = "CLAP_COUTURE_TEST_TOKEN")]
    #[couture(prompt, secret)]
    token: SecretString,
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
}
