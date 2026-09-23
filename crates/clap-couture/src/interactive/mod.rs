//! Ask on the terminal for marked args the user left out.
//!
//! Mark a field with `#[couture(prompt)]` to ask its help text, or `#[couture(prompt = "...")]`
//! to ask something else. When the user leaves the arg out and stdin and stderr are terminals,
//! [`CoutureParser`](crate::CoutureParser)'s entry points ask for it, then parse the answer as if
//! it had been typed. Without a terminal, parsing behaves exactly like clap.
//!
//! ```no_run
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Region to deploy to
//!     #[arg(long)]
//!     #[couture(prompt)]
//!     region: String,
//! }
//!
//! let cli = Cli::couture_parse();
//! ```
//!
//! A marked arg that has a default is asked with the default pre-filled. An optional arg answered
//! with nothing stays unset. An arg passed on the command line or through its env var is never
//! asked.
//!
//! A mark that can never become a single prompt fails to compile:
//!
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt)]
//!     tags: Vec<String>,
//! }
//! ```
//!
//! ```compile_fail
//! use clap::{Args, Parser};
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[command(flatten)]
//!     #[couture(prompt)]
//!     common: Common,
//! }
//!
//! #[derive(Args, Couture)]
//! struct Common {
//!     #[arg(long)]
//!     name: String,
//! }
//! ```

#[cfg(feature = "interactive-cliclack")]
mod cliclack;
#[cfg(feature = "interactive-cliclack")]
mod console_style;
mod flow;
mod tree;

use std::io::{self, IsTerminal as _};

use clap::builder::{PossibleValue, Styles};
#[cfg(feature = "interactive-cliclack")]
pub use cliclack::Cliclack;
pub(crate) use flow::run;
#[doc(hidden)]
pub use tree::{Mark, PromptChild, PromptNode, PromptSpec};

/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(feature = "interactive-cliclack")]
pub type DefaultPrompter = Cliclack;

/// A yes/no question.
#[derive(Debug)]
pub struct ConfirmPrompt<'prompt> {
    /// The answer Enter accepts.
    pub default: bool,
    /// clap's complaint about the previous answer, if any.
    pub error: Option<&'prompt str>,
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: &'prompt Styles,
}

/// Why a prompt produced no answer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PromptError {
    #[error("prompt cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("not a terminal")]
    NotATerminal,
}

/// A terminal UI that can ask for one arg's value.
///
/// Implement it to bring your own UI, or to answer from a script in tests.
pub trait Prompter {
    /// Ask a yes/no question.
    ///
    /// # Errors
    /// [`PromptError::Cancelled`] when the user backs out.
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError>;

    /// Check whether there is a terminal to ask on: stdin and stderr must both be one.
    fn is_available(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    /// Ask to pick one of `prompt.options`, returning the picked option's name.
    ///
    /// # Errors
    /// [`PromptError::Cancelled`] when the user backs out.
    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<String, PromptError>;

    /// Ask for free text, returning `None` only for an empty answer to an optional prompt.
    ///
    /// # Errors
    /// [`PromptError::Cancelled`] when the user backs out.
    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError>;
}

/// A pick among an arg's possible values.
#[derive(Debug)]
pub struct SelectPrompt<'prompt> {
    /// The name of the option Enter accepts, if any.
    pub default: Option<&'prompt str>,
    /// clap's complaint about the previous answer, if any.
    pub error: Option<&'prompt str>,
    /// The visible possible values, in declared order.
    pub options: &'prompt [PossibleValue],
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: &'prompt Styles,
}

/// A free-text question.
#[derive(Debug)]
pub struct TextPrompt<'prompt> {
    /// The answer Enter accepts, if any.
    pub default: Option<&'prompt str>,
    /// clap's complaint about the previous answer, if any.
    pub error: Option<&'prompt str>,
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: &'prompt Styles,
}

/// Map a backend's I/O error: `Interrupted` is the user backing out, `NotConnected` a missing
/// terminal.
#[cfg(feature = "interactive-cliclack")]
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
