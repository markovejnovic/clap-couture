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
#[cfg(feature = "interactive-inquire")]
mod inquire;
mod non_interactive;

use std::io::{self, IsTerminal as _, Write as _};

use clap::builder::Styles;
#[cfg(feature = "interactive-cliclack")]
pub use cliclack::Cliclack;
#[cfg(feature = "interactive-dialoguer")]
pub use dialoguer::Dialoguer;
#[cfg(feature = "interactive-inquire")]
pub use inquire::Inquire;
pub use non_interactive::NonInteractive;

#[cfg(feature = "interactive-editor")]
use super::EditorPrompt;
use super::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

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
    /// The default opens the editor through [`edit`](crate::interactive::edit), asking with
    /// [`Backend::confirm`] whether the user is done when the editor returns right away. Override
    /// it to draw something around the editor, and call `edit` from the override.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the editor dies of Ctrl-C.
    /// - [`PromptError::Io`] when the editor cannot be found or started, or exits unsuccessfully.
    #[cfg(feature = "interactive-editor")]
    fn editor(&self, prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
        super::edit(prompt, |question| {
            self.confirm(&ConfirmPrompt { default: true, question, styles: prompt.styles })
        })
    }

    /// Check whether there is a terminal to ask on: stdin and stderr must both be one.
    fn is_available(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    /// Show clap's complaint about the previous answer, before asking again.
    ///
    /// # Errors
    /// - [`PromptError::Io`] when the message cannot be shown.
    fn report(&self, error: &str, _styles: CommandStyles<'_>) -> Result<(), PromptError> {
        writeln!(io::stderr(), "{error}").map_err(PromptError::Io)
    }

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
