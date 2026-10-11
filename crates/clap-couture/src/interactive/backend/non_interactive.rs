//! A [`Backend`] that never asks.

use super::Backend;
use crate::interactive::{ConfirmPrompt, MultiSelectPrompt, PromptError, SelectPrompt, TextPrompt};

/// Never asks, as if there were no terminal: every arg is left to clap.
#[derive(Clone, Copy, Debug, Default)]
pub struct NonInteractive;

impl Backend for NonInteractive {
    fn confirm(&self, _prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Err(PromptError::NotATerminal)
    }

    fn is_available(&self) -> bool {
        false
    }

    fn multi_select(&self, _prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        Err(PromptError::NotATerminal)
    }

    fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(PromptError::NotATerminal)
    }

    fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(PromptError::NotATerminal)
    }
}
