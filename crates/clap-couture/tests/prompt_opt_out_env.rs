//! `NO_PROMPT` and `CI` turn prompts off, leaving missing args to clap. This test sets
//! process-wide variables, so it lives alone in its own binary.
#![cfg(feature = "interactive")]

use clap::{Parser, error::ErrorKind};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{
        Backend, ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt,
        TextPrompt, secrecy::SecretString,
    },
};

#[derive(Debug, Parser, Couture)]
struct Cli {
    #[arg(long)]
    #[couture(prompt)]
    region: String,
}

/// Answers every prompt, but leaves whether it may ask to [`Backend::is_available`]'s default.
struct Answers;

impl Backend for Answers {
    fn confirm(&self, _prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Ok(true)
    }

    fn multi_select(&self, _prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        Ok(Vec::new())
    }

    fn secret(&self, _prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
        Ok(Some(SecretString::from("asked")))
    }

    fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Ok(None)
    }

    fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        Ok(Some("asked".to_owned()))
    }
}

#[test]
fn an_opt_out_leaves_missing_args_to_clap() {
    let opt_outs = [("NO_PROMPT", "1"), ("CI", "true")];
    for (variable, value) in opt_outs {
        // Only `variable` is set, as `CI` may already be where this test runs.
        for (other, _) in opt_outs {
            // SAFETY: this binary holds a single test, so no other thread reads the environment.
            unsafe {
                std::env::remove_var(other);
            }
        }
        // SAFETY: as above.
        unsafe {
            std::env::set_var(variable, value);
        }
        let kind =
            Cli::couture_try_parse_from_with(Answers, ["app"]).err().and_then(|error| error.kind());
        assert_eq!(
            (variable, Answers.is_available(), kind),
            (variable, false, Some(ErrorKind::MissingRequiredArgument))
        );
    }
}
