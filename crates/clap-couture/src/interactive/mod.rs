//! Ask on the terminal for prompted args the user left out.
//!
//! Mark a field with `#[couture(prompt)]` to ask its help text, or `#[couture(prompt = "...")]`
//! to ask something else; with `markdown`, either is rendered as help text is. When the user leaves
//! the arg out and stdin and stderr are terminals, [`CoutureParser`](crate::CoutureParser)'s entry
//! points ask for it, then parse the answer as if it had been typed. Without a terminal, parsing
//! behaves exactly like clap.
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
//! answer, or until its `num_args` is reached; fewer than `num_args` asks for more.
//! An arg passed on the command line or through its env var is never
//! asked, and neither is one that conflicts with a passed arg or an earlier answer. An arg
//! required through `required_unless_present` or a required `ArgGroup` is asked as required, and
//! an empty answer is refused; in a group of prompted args, only the last one asked is. clap can't
//! lift `required_unless_present_all` or `required_if_eq` from an arg, so when one of those
//! fires, parsing fails with clap's error before anything is asked. Answers are not re-checked
//! against `requires` and similar rules between args.
//!
//! `#[couture(confirm)]` on a `bool` flag the user left out asks whether to go ahead, after every
//! other prompt, with each `{id}` in the question replaced by arg `id`'s value. A yes sets the
//! flag; a no fails the parse. Without a terminal, the flag must be passed, as if it were required.
//!
//! A yes sets the flag to `true`, so a flag clap sets to `false` can't be confirmed:
//! ```compile_fail
//! use clap::{ArgAction, Parser};
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long = "no-verify", action = ArgAction::SetFalse)]
//!     #[couture(confirm)]
//!     verify: bool,
//! }
//! ```
//!
//! # Answering in the editor
//!
//! With the `interactive-editor` feature, `#[couture(prompt, editor)]` asks in the user's editor
//! instead, for text a single line can't hold. `editor(extension = "md")` names the temp file's
//! extension, `txt` by default, so the editor highlights it. `editor(initial = "...")` is the text
//! the file starts with; an expression other than a string literal is evaluated each time.
#![cfg_attr(feature = "interactive-editor", doc = "```no_run")]
#![cfg_attr(not(feature = "interactive-editor"), doc = "```ignore")]
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Release notes
//!     #[arg(long)]
//!     #[couture(prompt, editor(extension = "md", initial = "## Changes\n\n## Breaking"))]
//!     notes: String,
//! }
//!
//! let cli = Cli::couture_parse();
//! ```
//! 
//! The editor is found as git finds its own: `$VISUAL` (unless `TERM` says the terminal is
//! dumb), then `$EDITOR`, then `vi`, and `:` takes the file as written. Its string is split into
//! words like a shell would, so `EDITOR="code --wait"` works, but `~` and variables in it are not
//! expanded. A known editor that returns at once, like `code`, gets the flag that makes it wait;
//! any other that returns right away without touching the file is asked about before its answer
//! is taken.
//!
//! The file opens with the answer clap just turned down, else the arg's default, else the
//! `initial` text, above a header asking the question. The header's lines start with `#|`, so it
//! comes back out while the user's own `#` lines, like markdown headings, stay. An empty file leaves an optional arg unset
//! and aborts for a required one, like an empty commit message. It aborts for an arg with a
//! default too, rather than quietly bringing the default back. The file is read in the encoding
//! its byte order mark names, or as UTF-8, or else in the system's legacy encoding: the ANSI code
//! page on Windows, the locale's elsewhere. A file in none of them reopens with an error.
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
//! 
//! nor does a flag clap counts:
#![cfg_attr(feature = "interactive-editor", doc = "```compile_fail")]
#![cfg_attr(not(feature = "interactive-editor"), doc = "```ignore")]
//! use clap::{ArgAction, Parser};
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(short, action = ArgAction::Count)]
//!     #[couture(prompt, editor)]
//!     verbose: u8,
//! }
//! ```
//! 
//! # Secrets
//!
//! Add `secret` to ask for a [`secrecy::SecretString`] without echoing it. A prompted answer
//! never reaches clap as plaintext; a value from the env var or an allowed command line is held
//! by clap like any arg's. The arg refuses a value from the command line, where `ps` and shell
//! history would see it; `secret(allow_argv)` allows it. A value from its env var is used without
//! asking, and `--help` names the variable but hides its value. An `Option<SecretString>` field
//! makes the secret optional: left empty, it stays `None`.
//!
//! Only [`CoutureParser`](crate::CoutureParser)'s entry points and
//! [`couture_command`](crate::CoutureParser::couture_command) guard secret args; plain
//! `Cli::parse()` or `Cli::command()` takes `--token` from the command line and shows its env
//! value in `--help`.
//! ```
//! use clap::Parser;
//! use clap_couture::{Couture, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// API token
//!     #[arg(long, env = "API_TOKEN")]
//!     #[couture(prompt, secret)]
//!     token: SecretString,
//! }
//! ```
//! 
//! `secret` modifies a prompt, so it needs one:
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::{Couture, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(secret)]
//!     token: SecretString,
//! }
//! ```
//! 
//! A secret takes no default, which would be baked into the binary:
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::{Couture, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long, default_value = "hunter2")]
//!     #[couture(prompt, secret)]
//!     token: SecretString,
//! }
//! ```
//! 
//! and its field must be a `SecretString`:
//! ```compile_fail,E0277
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt, secret)]
//!     token: String,
//! }
//! ```
//! 
//! A confirmation's question can't name a secret, which would show its value; a computed question
//! that does shows `********` in its place:
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::{Couture, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt, secret)]
//!     token: SecretString,
//!     #[arg(long)]
//!     #[couture(confirm = "Use token {token}?")]
//!     yes: bool,
//! }
//! ```
//! 
//! A required secret left out with no terminal to ask on fails the parse naming its env var, not
//! the flag the command line can't pass it with; usage leaves that flag out too.
//!
//! # Reading stdin
//!
//! `#[couture(stdin)]` reads all of stdin into an arg passed `-`, as in `kubectl apply -f -`;
//! see [Dash as a value](#dash-as-a-value). One trailing line ending is dropped, so `echo eu |`
//! reads as `eu`. What is read must be UTF-8, and at most 16 MiB: more fails the parse rather
//! than exhausting memory.
//!
//! `#[couture(stdin(piped))]` also takes stdin when the arg is left out and stdin is a pipe (or a
//! socket, as some runtimes pipe through one), so `echo hi | post` works like `post --message
//! hi`. A terminal, a file or the null device is never read. Nothing piped, or an arg passed on
//! the command line or through its env var, leaves stdin unread, and so does a passed arg it
//! conflicts with. Stdin is read only once the rest of the command line parses, so `--help` never
//! waits on it.
//!
//! <div class="warning">
//!
//! A pipe on stdin is no sign that anything was piped for the CLI, and `stdin(piped)` reads it to
//! its end. Run as `ssh host cli` without `-t`, from Node's `spawn` with its default piped stdin,
//! or under mintty on Windows, which shows a terminal as a pipe, the parse hangs until the user
//! closes stdin. Inside `while read line; do cli; done < file`, it eats the rest of the loop's
//! input. Prefer `stdin`, where the user asks for stdin with `-`.
//!
//! </div>
//!
//! A piped value is never asked for, so beside `prompt` the arg is asked only when nothing is
//! piped. Beside `secret`, the piped value is held like a prompted answer, never as plain text in
//! clap.
//! ```
//! use clap::Parser;
//! use clap_couture::{Couture, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Password
//!     #[arg(long)]
//!     #[couture(prompt, secret, stdin(piped))]
//!     password: SecretString,
//! }
//! ```
//! 
//! Stdin can only be read once, so at most one arg on any command path takes it. Each subcommand
//! may have one of its own, but not alongside one on a command above it:
//! ```compile_fail,E0080
//! use clap::{Parser, Subcommand};
//! use clap_couture::{Couture, CoutureParser as _};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(stdin(piped))]
//!     subject: Option<String>,
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[derive(Subcommand, Couture)]
//! enum Cmd {
//!     Send {
//!         #[arg(long)]
//!         #[couture(stdin(piped))]
//!         body: String,
//!     },
//! }
//!
//! let cli = Cli::couture_parse();
//! ```
//! 
//! An arg taking stdin takes exactly one value, so a flag or a `Vec` can't:
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(stdin(piped))]
//!     tags: Vec<String>,
//! }
//! ```
//! 
//! That holds for a flag given its action too:
//! ```compile_fail
//! use clap::{ArgAction, Parser};
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long, action = ArgAction::SetTrue)]
//!     #[couture(stdin(piped))]
//!     force: bool,
//! }
//! ```
//! 
//! # Dash as a value
//!
//! `dash` makes `-` as an arg's value stand for something, as in `git commit -F -`.
//! `#[couture(prompt(dash))]` asks for the arg when its value is `-`, and only then: left out, it
//! stays unset. `#[couture(stdin)]` reads all of stdin into the arg when its value is `-`,
//! whatever stdin is, a pipe, a file or a terminal (read until end of input), and leaves stdin
//! alone otherwise. Either works for a flag's value (`--message -`, `--message=-`, `-m-`,
//! `-vm -`) or a positional, beside `secret` too: the `-` is no secret on the command line.
//! `prompt(dash, question = "...")` asks its own question.
//! ```
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser as _, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Commit message; `-` asks for it.
//!     #[arg(short, long)]
//!     #[couture(prompt(dash), editor)]
//!     message: Option<String>,
//!
//!     /// Password; `-` reads it from stdin.
//!     #[arg(long)]
//!     #[couture(prompt, secret, stdin)]
//!     password: Option<SecretString>,
//! }
//!
//! let cli = Cli::couture_parse_from(["commit", "-m", "Fix login"]);
//! # assert_eq!(cli.message.as_deref(), Some("Fix login"));
//! ```
//! 
//! With no terminal to ask on, a `-` that asks fails the parse.
//!
//! With both, `-` asks at a terminal and reads stdin otherwise, so `--password -` asks masked when
//! typed, and takes the password when it is piped in. A terminal there is no asking on, as with
//! [`NonInteractive`], is read until end of input.
//! ```
//! use clap::Parser;
//! use clap_couture::{Couture, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Password; `-` asks for it, or reads it from a pipe.
//!     #[arg(long)]
//!     #[couture(prompt(dash), secret, stdin)]
//!     password: Option<SecretString>,
//! }
//! ```
//! 
//! `-` stands for the arg's one value, so a flag or a `Vec` can't take it:
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt(dash))]
//!     tags: Vec<String>,
//! }
//! ```
//! 
//! and neither can a flag clap counts or sets, whatever its type:
//! ```compile_fail
//! use clap::{ArgAction, Parser};
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(short, action = ArgAction::Count)]
//!     #[couture(prompt(dash))]
//!     verbose: u8,
//! }
//! ```

mod backend;
pub(crate) mod prompted_args;
mod prompter;
mod sealed;

use std::io;

#[cfg(feature = "interactive-cliclack")]
pub use backend::Cliclack;
#[cfg(feature = "interactive-dialoguer")]
pub use backend::Dialoguer;
#[cfg(feature = "interactive-editor")]
pub use backend::EditorInteraction;
#[cfg(feature = "interactive-inquire")]
pub use backend::Inquire;
pub use backend::{Backend, CommandStyles, DefaultBackend, NonInteractive};
use clap::builder::PossibleValue;
#[cfg(feature = "interactive-editor")]
pub(crate) use prompted_args::EditorAttr;
pub(crate) use prompted_args::{DashAttr, PromptPurpose, PromptedArg, PromptedArgTree, StdinArg};
pub use prompter::Prompter;
/// The [`secrecy`](https://docs.rs/secrecy) crate couture's secret args are typed with, so the app
/// names the same `SecretString`.
pub use secrecy;

pub use crate::os::signal::Signal;

/// Why a prompt produced no answer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PromptError {
    /// The user backed out, which counts as Ctrl-C.
    #[error("prompt cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(io::Error),
    #[error("not a terminal")]
    NotATerminal,
    /// The editor died of this Unix signal, like Ctrl-C or the terminal closing. The prompt is
    /// cancelled, and [`couture_parse`](crate::CoutureParser::couture_parse) dies of the same
    /// signal, so a shell loop around the CLI stops too.
    #[error("prompt cancelled")]
    Signalled(Signal),
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
#[non_exhaustive]
pub struct ConfirmPrompt<'prompt> {
    /// The answer Enter accepts.
    pub default: bool,
    /// What to ask; with `markdown`, it may hold ANSI escapes.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A question answered in the user's editor, for `#[couture(prompt, editor)]`.
#[cfg(feature = "interactive-editor")]
#[derive(Debug)]
#[non_exhaustive]
pub struct EditorPrompt<'prompt> {
    /// clap's complaint about the previous answer, shown in the file's header.
    pub error: Option<&'prompt str>,
    /// The temp file's extension, without the dot.
    pub extension: &'prompt str,
    /// The temp file's name, before the extension: the command path and the arg's id.
    pub file_stem: &'prompt str,
    /// The text the file starts with, for the user to keep, edit or clear: the previous rejected
    /// answer, the arg's default, or the `initial` text it was given.
    pub initial: Option<&'prompt str>,
    /// The header's last line: how to cancel, or how to leave the arg unset.
    pub instructions: &'prompt str,
    /// The error the file reopens with when it was saved in an encoding that can't be read.
    pub not_utf8: &'prompt str,
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask; with `markdown`, it may hold ANSI escapes.
    pub question: &'prompt str,
    /// Asked when the editor returned at once and left the file as it was: done editing?
    pub returned_early: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
    /// The error the file reopens with while it still holds the U+FFFD marking unreadable text.
    pub unreadable: &'prompt str,
    /// The hint shown while the editor is open; empty shows none.
    pub waiting: &'prompt str,
}

/// A pick of any number of an arg's possible values.
#[derive(Debug)]
#[non_exhaustive]
pub struct MultiSelectPrompt<'prompt> {
    /// The names of the options picked to begin with.
    pub defaults: &'prompt [String],
    /// Picking none of the options is allowed.
    pub optional: bool,
    /// The visible possible values, in declared order.
    pub options: &'prompt [PossibleValue],
    /// What to ask; with `markdown`, it may hold ANSI escapes.
    pub question: &'prompt str,
    /// What to tell the user when they select nothing, unless the prompt is optional.
    pub selection_required: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A question whose answer is typed masked and never echoed.
#[derive(Debug)]
#[non_exhaustive]
pub struct SecretPrompt<'prompt> {
    /// An empty answer is allowed, and leaves the arg unset.
    pub optional: bool,
    /// What to ask; with `markdown`, it may hold ANSI escapes.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A pick among an arg's possible values.
#[derive(Debug)]
#[non_exhaustive]
pub struct SelectPrompt<'prompt> {
    /// The name of the option Enter accepts, if any.
    pub default: Option<&'prompt str>,
    /// The extra option an optional prompt lists to pick none of the options.
    pub none_option: &'prompt str,
    /// Picking none of the options is allowed, and leaves the arg unset.
    pub optional: bool,
    /// The visible possible values, in declared order.
    pub options: &'prompt [PossibleValue],
    /// What to ask; with `markdown`, it may hold ANSI escapes.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}

/// A free-text question.
#[derive(Debug)]
#[non_exhaustive]
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
    /// What to ask; with `markdown`, it may hold ANSI escapes.
    pub question: &'prompt str,
    /// The command's styles, for backends that theme themselves.
    pub styles: CommandStyles<'prompt>,
}
