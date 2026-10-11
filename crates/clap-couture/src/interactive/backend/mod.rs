//! The terminal UIs that ask for prompted args.

#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
mod builtin;
#[cfg(feature = "interactive-cliclack")]
mod cliclack;
#[cfg(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))]
mod console_style;
#[cfg(feature = "interactive-dialoguer")]
mod dialoguer;
#[cfg(feature = "interactive-editor")]
mod editor_interaction;
#[cfg(feature = "interactive-inquire")]
mod inquire;
mod non_interactive;

use std::io::{self, IsTerminal as _, Write as _};

use clap::builder::Styles;
#[cfg(feature = "interactive-cliclack")]
pub use cliclack::Cliclack;
#[cfg(feature = "interactive-dialoguer")]
pub use dialoguer::Dialoguer;
#[cfg(feature = "interactive-editor")]
pub use editor_interaction::EditorInteraction;
#[cfg(feature = "interactive-inquire")]
pub use inquire::Inquire;
pub use non_interactive::NonInteractive;
use secrecy::SecretString;

#[cfg(feature = "interactive-editor")]
use super::EditorPrompt;
use super::{
    ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt, TextPrompt,
};

/// A command's clap styles, for a backend to build its theme from.
#[derive(Clone, Copy, Debug)]
pub struct CommandStyles<'styles>(pub &'styles Styles);

/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire, or [`NonInteractive`] when none is.
#[cfg(feature = "interactive-cliclack")]
pub type DefaultBackend = Cliclack;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire, or [`NonInteractive`] when none is.
#[cfg(all(feature = "interactive-dialoguer", not(feature = "interactive-cliclack")))]
pub type DefaultBackend = Dialoguer;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire, or [`NonInteractive`] when none is.
#[cfg(all(
    feature = "interactive-inquire",
    not(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))
))]
pub type DefaultBackend = Inquire;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire, or [`NonInteractive`] when none is.
#[cfg(not(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
)))]
pub type DefaultBackend = NonInteractive;

/// A terminal UI that can ask for one arg's value.
///
/// Implement it to bring your own UI, or to answer from a script in tests.
pub trait Backend {
    /// Ask a yes/no question.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError>;

    /// Ask in the user's editor, returning `None` for an empty answer. Whether an empty answer is
    /// allowed is decided by the caller, from `prompt.optional`.
    ///
    /// The default opens the editor through
    /// [`EditorInteraction`](crate::interactive::EditorInteraction), asking with
    /// [`Backend::confirm`] whether the user is done when the editor returns right
    /// away. Override it to draw something around the editor, and use `EditorInteraction` from the
    /// override.
    ///
    /// # Errors
    /// - [`PromptError::Signalled`] when the editor dies of Ctrl-C or of the terminal closing;
    ///   [`PromptError::Cancelled`] on Windows.
    /// - [`PromptError::Cancelled`] when the done-editing question is cancelled.
    /// - [`PromptError::Io`] when the editor cannot be found or started, or exits unsuccessfully.
    #[cfg(feature = "interactive-editor")]
    fn editor(&self, prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
        EditorInteraction::start(self, prompt)?.answer()
    }

    /// Check whether there is a terminal to ask on: stdin and stderr must both be one.
    fn is_available(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    /// Ask to pick any number of `prompt.options`, returning the picked options' names in
    /// declared order. Unless `prompt.optional`, at least one must be picked.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn multi_select(&self, prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError>;

    /// Show clap's complaint about the previous answer, before asking again.
    ///
    /// # Errors
    /// - [`PromptError::Io`] when the message cannot be shown.
    fn report(&self, error: &str, _styles: CommandStyles<'_>) -> Result<(), PromptError> {
        writeln!(io::stderr(), "{error}").map_err(PromptError::Io)
    }

    /// Ask for a secret without echoing it, returning `None` only for an empty answer to an
    /// optional prompt.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn secret(&self, prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError>;

    /// Ask to pick one of `prompt.options`, returning the picked option's name, or `None` when an
    /// optional prompt is answered with none of them.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError>;

    /// Ask for free text, returning `None` only for an empty answer to an optional prompt.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError>;
}

impl<B> Backend for &B
where
    B: Backend + ?Sized,
{
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        (**self).confirm(prompt)
    }

    #[cfg(feature = "interactive-editor")]
    fn editor(&self, prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
        (**self).editor(prompt)
    }

    fn is_available(&self) -> bool {
        (**self).is_available()
    }

    fn multi_select(&self, prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        (**self).multi_select(prompt)
    }

    fn report(&self, error: &str, styles: CommandStyles<'_>) -> Result<(), PromptError> {
        (**self).report(error, styles)
    }

    fn secret(&self, prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
        (**self).secret(prompt)
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        (**self).select(prompt)
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        (**self).text(prompt)
    }
}
