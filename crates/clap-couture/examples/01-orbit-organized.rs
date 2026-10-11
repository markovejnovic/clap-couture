//! Demo of clap-couture's categories.
//!
//! clap-couture enables you to organize your subcommands in categories.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 01-orbit-organized -- --help` outputs:
//!
//! ```terminal
//! Deploy and manage your apps from the terminal
//!
//! Usage:
//!   01-orbit-organized <COMMAND>
//!
//!   logs      Tail live logs
//!   search    Search your deployment history
//!   status    Show recent activity
//!
//! Deploy:
//!   deploy    Ship the current project
//!   rollback  Roll back to a previous release
//!   secrets   Manage environment secrets
//!
//! Account & sync:  Who you are, on every machine
//!   login     Sign in to your account
//!   sync      Sync settings across machines
//!
//! Options:
//!   -h, --help  Print help
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};

// You need to decorate your top-level CLI entrypoint with `#[derive(Couture)]`.
// Couture will preserve all of what clap does, including rsdoc!
/// Deploy and manage your apps from the terminal.
#[derive(Parser, Couture)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

// You also need to decorate all your subcommands with `#[derive(Couture)]`.
#[derive(Subcommand, Couture)]
#[couture(categories = {
    "deploy" = { title = "Deploy:" },
    // `title` replaces the default `deploy:`/`account:` heading, and `description` goes beside it.
    "account" = { title = "Account & sync:", description = "Who you are, on every machine" }
})]
enum Cmd {
    /// Ship the current project.
    #[couture(category = "deploy")]
    Deploy,
    /// Sign in to your account.
    #[couture(category = "account")]
    Login,
    // Uncategorized entries go into an uncategorized section at the top. In this example, that's
    // Logs, Search, and Status.
    /// Tail live logs.
    Logs,
    /// Roll back to a previous release.
    #[couture(category = "deploy")]
    Rollback,
    /// Search your deployment history.
    Search,
    /// Manage environment secrets.
    #[couture(category = "deploy")]
    Secrets,
    /// Show recent activity.
    Status,
    /// Sync settings across machines.
    #[couture(category = "account")]
    Sync,
}

fn main() {
    // You need to change the `Cli::parse` call to `Cli::couture_parse()`.
    let cli = Cli::couture_parse();

    let ran = match cli.cmd {
        Cmd::Search => "search",
        Cmd::Status => "status",
        Cmd::Logs => "logs",
        Cmd::Deploy => "deploy",
        Cmd::Rollback => "rollback",
        Cmd::Secrets => "secrets",
        Cmd::Login => "login",
        Cmd::Sync => "sync",
    };
    println!("running: {ran}");
}
