//! Demo of everything clap-couture can do, at once.
//!
//! The same `orbit` CLI as the other examples, with every feature turned on: categories with
//! markdown descriptions, commands with no category, hidden commands, nested and flattened
//! subcommands, interactive prompts through a backend of its own, and custom headings.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 09-interactive-custom-backend -- --help` outputs:
//!
//! ```terminal
//! Deploy and manage your apps from the terminal
//!
//! Usage:
//!   09-interactive-custom-backend <COMMAND>
//!
//!   status    Show recent activity
//!
//! Everyday:        What you'll reach for daily
//!   logs      Tail live logs
//!   search    Search your deployment history
//!
//! Deploy:          Ship code to production
//!   deploy    Ship the current project
//!   rollback  Roll back to a previous release
//!   secrets   Manage environment secrets
//!
//! Account & sync:
//!   login     Sign in to your account
//!   sync      Sync settings across machines
//!
//! Flags:
//!   -h, --help  Print help
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use std::io::{self, Write as _};

use clap::{Parser, Subcommand, ValueEnum, builder::PossibleValue};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{Backend, ConfirmPrompt, PromptError, SelectPrompt, TextPrompt},
};

/// Deploy and manage your apps from the terminal.
#[derive(Parser, Couture)]
// couture prints a few headings of its own; `messages` replaces them, eg. to localize them.
#[couture(messages = { options = "Flags:" })]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    // Descriptions are markdown, rendered in the terminal.
    "everyday" = { title = "Everyday:", description = "What you'll reach for **daily**" },
    "deploy"   = { title = "Deploy:", description = "Ship code to *production*" },
})]
enum Cmd {
    // A flattened enum's commands join this list, and bring their categories with them.
    #[command(flatten)]
    Account(Account),

    // Hidden commands are left out of the menu, as clap leaves them out of its own.
    /// Print orbit's internal state.
    #[command(hide = true)]
    Debug,

    /// Ship the current project.
    #[category("deploy")]
    Deploy(Deploy),

    /// Tail live logs.
    #[category("everyday")]
    Logs,

    /// Roll back to a previous release.
    #[category("deploy")]
    Rollback,

    /// Search your deployment history.
    #[category("everyday")]
    Search,

    /// Manage environment secrets.
    #[category("deploy")]
    #[command(subcommand)]
    Secrets(Secrets),

    // A command without a `#[category]` is listed first, under no heading.
    /// Show recent activity.
    Status,
}

#[derive(Parser, Couture)]
struct Deploy {
    /// Region to deploy to.
    #[arg(long, value_enum)]
    // Leave `--region` out on a terminal, and orbit asks for it, through `Plain` below.
    #[couture(prompt = "Which region?")]
    region: Region,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Region {
    Apac,
    Eu,
    Us,
}

// Nested subcommands are clap's as usual; couture organizes the top-level menu.
#[derive(Subcommand)]
enum Secrets {
    /// List the secrets.
    List,

    /// Set a secret.
    Set,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "account" = { title = "Account & sync:" } })]
enum Account {
    /// Sign in to your account.
    #[category("account")]
    Login,

    /// Sync settings across machines.
    #[category("account")]
    Sync,
}

/// A prompt backend of your own: plain questions on stderr, answered on stdin, for
/// terminals that don't take well to cliclack's interactive widgets.
struct Plain;

impl Plain {
    /// Ask `question`, returning the line typed in answer, trimmed.
    fn ask(question: &str) -> Result<String, PromptError> {
        let mut stderr = io::stderr();
        write!(stderr, "{question} ")?;
        stderr.flush()?;

        let mut line = String::new();
        // No line at all means stdin closed, eg. on Ctrl-D: the user backed out.
        if io::stdin().read_line(&mut line)? == 0 {
            return Err(PromptError::Cancelled);
        }
        Ok(line.trim().to_owned())
    }

    /// `answer`, or the default for an empty one. An empty answer to a required prompt
    /// without a default is returned as is, so clap's complaint about it is shown and the
    /// question asked again.
    fn or_default(answer: String, default: Option<&str>, optional: bool) -> Option<String> {
        if !answer.is_empty() {
            return Some(answer);
        }
        default.map(str::to_owned).or_else(|| (!optional).then_some(answer))
    }
}

impl Backend for Plain {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        let choices = if prompt.default { "[Y/n]" } else { "[y/N]" };
        let answer = Self::ask(&format!("{} {choices}", prompt.question))?;
        Ok(match answer.to_lowercase().as_str() {
            "" => prompt.default,
            answer => answer.starts_with('y'),
        })
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        let names: Vec<&str> = prompt.options.iter().map(PossibleValue::get_name).collect();
        let answer = Self::ask(&format!("{} [{}]", prompt.question, names.join("/")))?;
        Ok(Self::or_default(answer, prompt.default, prompt.optional))
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let answer = Self::ask(prompt.question)?;
        Ok(Self::or_default(answer, prompt.default, prompt.optional))
    }
}

fn main() {
    // Parse with your own backend, in place of the default one `Cli::couture_parse()` uses.
    let cli = Cli::couture_try_parse_from_with(&Plain, std::env::args_os())
        .unwrap_or_else(|error| error.exit());
    let ran = match cli.cmd {
        Cmd::Status => "status",
        Cmd::Logs => "logs",
        Cmd::Search => "search",
        Cmd::Deploy(Deploy { region }) => match region {
            Region::Apac => "deploy to apac",
            Region::Eu => "deploy to eu",
            Region::Us => "deploy to us",
        },
        Cmd::Rollback => "rollback",
        Cmd::Secrets(Secrets::Set) => "secrets set",
        Cmd::Secrets(Secrets::List) => "secrets list",
        Cmd::Account(Account::Login) => "login",
        Cmd::Account(Account::Sync) => "sync",
        Cmd::Debug => "debug",
    };
    println!("running: {ran}");
}
