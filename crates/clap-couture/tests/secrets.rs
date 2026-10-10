//! `#[couture(prompt, secret)]` asks masked, keeps the answer out of clap, and refuses the command
//! line.
#![cfg(feature = "interactive")]

use clap::Parser;
use clap_couture::{Couture, interactive::secrecy::SecretString};
use rstest::rstest;

#[derive(Parser, Couture)]
struct Login {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret)]
    token: SecretString,
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
    assert_eq!(allow_argv(&ScriptedLogin::PROMPTED_ARGS), Some(true));
}
