//! The terminal UIs that ask for marked args.

#[cfg(feature = "interactive-cliclack")]
mod cliclack;
#[cfg(feature = "interactive-dialoguer")]
mod dialoguer;
#[cfg(feature = "interactive-inquire")]
mod inquire;

use std::io::{self, IsTerminal as _, Write as _};

use clap::builder::Styles;
#[cfg(feature = "interactive-cliclack")]
pub use cliclack::Cliclack;
#[cfg(feature = "interactive-dialoguer")]
pub use dialoguer::Dialoguer;
#[cfg(feature = "interactive-inquire")]
pub use inquire::Inquire;

use super::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

/// The item a backend adds to an optional select; picking it leaves the arg unset.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
const NONE_OPTION: &str = "(none)";

/// A command's clap styles, for a backend to build its theme from.
#[derive(Clone, Copy, Debug)]
pub struct CommandStyles<'styles>(pub &'styles Styles);

/// A clap ANSI color, for a backend to convert into its own color type.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub(crate) struct NamedColor(pub(crate) clap::builder::styling::AnsiColor);

/// A clap color, for a backend to convert into its own color type.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub(crate) struct ClapColor(pub(crate) clap::builder::styling::Color);

/// A clap style, for a backend to convert into its own style type.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub(crate) struct ClapStyle<'style>(pub(crate) &'style clap::builder::styling::Style);

/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(feature = "interactive-cliclack")]
pub type DefaultBackend = Cliclack;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(all(feature = "interactive-dialoguer", not(feature = "interactive-cliclack")))]
pub type DefaultBackend = Dialoguer;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(all(
    feature = "interactive-inquire",
    not(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))
))]
pub type DefaultBackend = Inquire;

/// A terminal UI that can ask for one arg's value.
///
/// Implement it to bring your own UI, or to answer from a script in tests.
pub trait Backend {
    /// Ask a yes/no question.
    ///
    /// # Errors
    /// [`PromptError::Cancelled`] when the user backs out.
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError>;

    /// Check whether there is a terminal to ask on: stdin and stderr must both be one.
    fn is_available(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    /// Show clap's complaint about the previous answer, before asking again.
    ///
    /// By default, `error` is written unstyled to stderr.
    ///
    /// # Errors
    /// [`PromptError::Io`] when the message cannot be shown.
    fn report(&self, error: &str, _styles: CommandStyles<'_>) -> Result<(), PromptError> {
        writeln!(io::stderr(), "{error}").map_err(PromptError::Io)
    }

    /// Ask to pick one of `prompt.options`, returning the picked option's name, or `None` when an
    /// optional prompt is answered with none of them.
    ///
    /// # Errors
    /// [`PromptError::Cancelled`] when the user backs out.
    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError>;

    /// Ask for free text, returning `None` only for an empty answer to an optional prompt.
    ///
    /// # Errors
    /// [`PromptError::Cancelled`] when the user backs out.
    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError>;
}

/// Map a backend's I/O error: `Interrupted` is the user backing out, `NotConnected` a missing
/// terminal.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "`io::ErrorKind` is #[non_exhaustive]; every other kind is a plain I/O failure"
)]
fn from_io(err: io::Error) -> PromptError {
    match err.kind() {
        io::ErrorKind::Interrupted => PromptError::Cancelled,
        io::ErrorKind::NotConnected => PromptError::NotATerminal,
        _ => PromptError::Io(err),
    }
}
