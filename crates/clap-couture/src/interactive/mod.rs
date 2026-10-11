//! Ask on the terminal for prompted args the user left out.
//!
//! Mark a field with `#[couture(prompt)]` to ask its help text, or `#[couture(prompt = "...")]`
//! to ask something else. When the user leaves the arg out and stdin and stderr are terminals,
//! [`CoutureParser`](crate::CoutureParser)'s entry points ask for it, then parse the answer as if
//! it had been typed. Without a terminal, parsing behaves exactly like clap.
//!
//! ```
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
//! let cli = Cli::couture_parse_from(["deploy", "--region", "eu"]);
//! assert_eq!(cli.region, "eu");
//! ```
//!
//! A prompted arg that has a default is asked with the default pre-filled. An optional arg answered
//! with nothing stays unset. An arg that takes many values, like a `Vec<T>`, is asked as a
//! multi-select when it has possible values, and otherwise one value at a time until an empty
//! answer. An arg passed on the command line or through its env var is never
//! asked, and neither is one that conflicts with a passed arg or an earlier answer. Answers are
//! not re-checked against `requires`, `required_if_eq` and similar rules between args.
//!
//! `#[couture(confirm)]` on a `bool` flag the user left out asks whether to go ahead, after every
//! other prompt, with each `{id}` in the question replaced by arg `id`'s value. A yes sets the
//! flag; a no fails the parse. Without a terminal, the flag must be passed, as if it were required.

mod backend;
mod prompted_args;

use std::io;

#[cfg(feature = "interactive-cliclack")]
pub use backend::Cliclack;
#[cfg(feature = "interactive-dialoguer")]
pub use backend::Dialoguer;
#[cfg(feature = "interactive-inquire")]
pub use backend::Inquire;
pub use backend::{Backend, CommandStyles, DefaultBackend, NonInteractive};
use clap::builder::PossibleValue;
#[doc(hidden)]
pub use prompted_args::{
    PromptAttr, PromptPurpose, PromptedArg, PromptedArgBranch, PromptedArgTree,
};

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

/// A pick of any number of an arg's possible values.
#[derive(Debug)]
pub struct MultiSelectPrompt<'prompt> {
    /// The names of the options picked to begin with.
    pub defaults: &'prompt [String],
    /// Picking none of the options is allowed.
    pub optional: bool,
    /// The visible possible values, in declared order.
    pub options: &'prompt [PossibleValue],
    /// What to ask.
    pub question: &'prompt str,
    /// What to tell the user when they select nothing, unless the prompt is optional.
    pub selection_required: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A pick among an arg's possible values.
#[derive(Debug)]
pub struct SelectPrompt<'prompt> {
    /// The name of the option Enter accepts, if any.
    pub default: Option<&'prompt str>,
    /// The extra option an optional prompt lists to pick none of the options.
    pub none_option: &'prompt str,
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
    /// What to show beside the question, if anything: for one more value of a multi-value arg,
    /// that an empty answer finishes the list.
    pub hint: Option<&'prompt str>,
    /// The text the answer starts out as, for the user to keep, edit or clear. Unlike
    /// [`default`](Self::default), clearing it leaves the answer empty.
    pub initial: Option<&'prompt str>,
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}
