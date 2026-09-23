//! An env var counts as passing the arg, so it suppresses the prompt. This test sets a
//! process-wide variable, so it lives alone in its own binary.
#![cfg(feature = "interactive")]

use std::io;

use clap::Parser;
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{ConfirmPrompt, PromptError, Prompter, SelectPrompt, TextPrompt},
};
use rstest::rstest;

#[derive(Parser, Couture)]
struct Cli {
    #[arg(long, env = "CLAP_COUTURE_TEST_REGION")]
    #[couture(prompt)]
    region: String,
}

/// A terminal that fails every prompt.
struct Refuses;

impl Prompter for Refuses {
    fn confirm(&self, _prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Err(refused())
    }

    fn is_available(&self) -> bool {
        true
    }

    fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(refused())
    }

    fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(refused())
    }
}

fn refused() -> PromptError {
    PromptError::Io(io::Error::other("nothing should be asked"))
}

#[rstest]
fn an_env_var_suppresses_the_prompt() {
    // SAFETY: this binary holds a single test, so no other thread reads the environment.
    unsafe {
        std::env::set_var("CLAP_COUTURE_TEST_REGION", "eu");
    }
    let region = Cli::couture_try_parse_from_with(&Refuses, ["app"]).map(|cli| cli.region);
    assert_eq!(region.ok().as_deref(), Some("eu"));
}
