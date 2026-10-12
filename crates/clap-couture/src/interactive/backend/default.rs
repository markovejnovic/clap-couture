//! The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through.

use secrecy::SecretString;

use super::{Backend, CommandStyles};
#[cfg(feature = "interactive-editor")]
use crate::interactive::EditorPrompt;
use crate::interactive::{
    ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt, TextPrompt,
};

/// The first enabled of cliclack, dialoguer and inquire, or
/// [`NonInteractive`](super::NonInteractive) when none is.
#[cfg(feature = "interactive-cliclack")]
type Chosen = super::Cliclack;
/// See the version with `interactive-cliclack` on.
#[cfg(all(feature = "interactive-dialoguer", not(feature = "interactive-cliclack")))]
type Chosen = super::Dialoguer;
/// See the version with `interactive-cliclack` on.
#[cfg(all(
    feature = "interactive-inquire",
    not(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))
))]
type Chosen = super::Inquire;
/// See the version with `interactive-cliclack` on.
#[cfg(not(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
)))]
type Chosen = super::NonInteractive;

/// Asks through the first enabled of cliclack, dialoguer and inquire, and never asks when none
/// is, as [`NonInteractive`](super::NonInteractive) does.
///
/// It is one type under every feature set, so enabling another backend anywhere in the dependency
/// tree changes the UI it shows, not code that names it. Name a backend, like
/// [`Cliclack`](super::Cliclack), to pin the UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultBackend(Chosen);

impl Backend for DefaultBackend {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        self.0.confirm(prompt)
    }

    #[cfg(feature = "interactive-editor")]
    fn editor(&self, prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
        self.0.editor(prompt)
    }

    fn is_available(&self) -> bool {
        self.0.is_available()
    }

    fn multi_select(&self, prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        self.0.multi_select(prompt)
    }

    fn report(&self, error: &str, styles: CommandStyles<'_>) -> Result<(), PromptError> {
        self.0.report(error, styles)
    }

    fn secret(&self, prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
        self.0.secret(prompt)
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        self.0.select(prompt)
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        self.0.text(prompt)
    }
}
