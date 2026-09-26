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
//! asked, and neither is one that conflicts with a passed arg or an earlier answer. Answers are
//! not re-checked against `requires`, `required_if_eq` and similar rules between args.
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

mod backend;
mod flow;
mod tree;

use std::io;

#[cfg(feature = "interactive-cliclack")]
pub use backend::Cliclack;
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub use backend::DefaultBackend;
#[cfg(feature = "interactive-dialoguer")]
pub use backend::Dialoguer;
#[cfg(feature = "interactive-inquire")]
pub use backend::Inquire;
pub use backend::{Backend, CommandStyles};
use clap::builder::PossibleValue;
pub(crate) use flow::run;
#[doc(hidden)]
pub use tree::{Mark, Probe, ProbeFallback, PromptChild, PromptNode, PromptSpec};

/// Why a prompt produced no answer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PromptError {
    #[error("prompt cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(io::Error),
    #[error("not a terminal")]
    NotATerminal,
}

// `Interrupted` is the user backing out, `NotConnected` a missing terminal.
impl From<io::Error> for PromptError {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "`io::ErrorKind` is #[non_exhaustive]; every other kind is a plain I/O failure"
    )]
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::Interrupted => Self::Cancelled,
            io::ErrorKind::NotConnected => Self::NotATerminal,
            _ => Self::Io(err),
        }
    }
}

/// A yes/no question.
#[derive(Debug)]
pub struct ConfirmPrompt<'prompt> {
    /// The answer Enter accepts.
    pub default: bool,
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A pick among an arg's possible values.
#[derive(Debug)]
pub struct SelectPrompt<'prompt> {
    /// The name of the option Enter accepts, if any.
    pub default: Option<&'prompt str>,
    /// Picking none of the options is allowed, and leaves the arg unset.
    pub optional: bool,
    /// The visible possible values, in declared order.
    pub options: &'prompt [PossibleValue],
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A free-text question.
#[derive(Debug)]
pub struct TextPrompt<'prompt> {
    /// The answer Enter accepts, if any.
    pub default: Option<&'prompt str>,
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}
