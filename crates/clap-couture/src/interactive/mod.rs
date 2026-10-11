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
//! with nothing stays unset. An arg passed on the command line or through its env var is never
//! asked, and neither is one that conflicts with a passed arg or an earlier answer. Answers are
//! not re-checked against `requires`, `required_if_eq` and similar rules between args.
//!
//! # Answering in the editor
//!
//! With the `interactive-editor` feature, `#[couture(prompt, editor)]` asks in the user's editor
//! instead, for text a single line can't hold. `editor(extension = "md")` names the temp file's
//! extension, `txt` by default, so the editor highlights it.
#![cfg_attr(feature = "interactive-editor", doc = "```no_run")]
#![cfg_attr(not(feature = "interactive-editor"), doc = "```ignore")]
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Release notes
//!     #[arg(long)]
//!     #[couture(prompt, editor(extension = "md"))]
//!     notes: String,
//! }
//!
//! let cli = Cli::couture_parse();
//! ```
//! 
//! The editor is found as git finds its own: `$VISUAL` (unless `TERM` says the terminal is
//! dumb), then `$EDITOR`, then `vi`, and `:` takes the file as written. Its string is split into
//! words like a shell would, so `EDITOR="code --wait"` works, but `~` and variables in it are not
//! expanded. An editor that returns right away without touching the file, like `code` without
//! `--wait`, is asked about before its answer is taken.
//!
//! The file opens with the arg's default, or the answer clap just turned down, above a header
//! asking the question. The header's lines start with `#|`, so it comes back out while the
//! user's own `#` lines, like markdown headings, stay. An empty file leaves an optional arg unset
//! and aborts for a required one, like an empty commit message. It aborts for an arg with a
//! default too, rather than quietly bringing the default back. A file saved in an encoding other
//! than UTF-8 reopens with an error, except that UTF-16 behind a byte order mark is read, and on
//! Windows so is the ANSI code page.
//!
//! `editor` needs `prompt` beside it:
#![cfg_attr(feature = "interactive-editor", doc = "```compile_fail")]
#![cfg_attr(not(feature = "interactive-editor"), doc = "```ignore")]
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(editor)]
//!     notes: String,
//! }
//! ```
//! 
//! and an arg that takes a value; a flag has nothing to edit:
#![cfg_attr(feature = "interactive-editor", doc = "```compile_fail")]
#![cfg_attr(not(feature = "interactive-editor"), doc = "```ignore")]
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt, editor)]
//!     follow: bool,
//! }
//! ```

mod backend;
#[cfg(feature = "interactive-editor")]
mod editor;
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
#[cfg(feature = "interactive-editor")]
pub use editor::edit;
#[doc(hidden)]
pub use prompted_args::{EditorAttr, PromptAttr, PromptedArg, PromptedArgBranch, PromptedArgTree};

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

/// A question answered in the user's editor, for `#[couture(prompt, editor)]`.
#[cfg(feature = "interactive-editor")]
#[derive(Debug)]
pub struct EditorPrompt<'prompt> {
    /// The text the file starts with: the arg's default, or the previous rejected answer.
    pub default: Option<&'prompt str>,
    /// clap's complaint about the previous answer, shown in the file's header.
    pub error: Option<&'prompt str>,
    /// The temp file's extension, without the dot.
    pub extension: &'prompt str,
    /// The temp file's name, before the extension: the command path and the arg's id.
    pub file_stem: &'prompt str,
    /// The header's last line: how to cancel, or how to leave the arg unset.
    pub instructions: &'prompt str,
    /// The error the file reopens with when it was saved in an encoding that can't be read.
    pub not_utf8: &'prompt str,
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask.
    pub question: &'prompt str,
    /// Asked when the editor returned at once and left the file as it was: done editing?
    pub returned_early: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
    /// Put before the setting that makes an editor known to return at once wait.
    pub wait_hint: &'prompt str,
    /// The hint shown while the editor is open.
    pub waiting: &'prompt str,
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
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}
