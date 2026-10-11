//! A minimal example of what programs look like without clap-couture.
//!
//! **This example does not use clap-couture and is intended as a "before" render.**
//!
//! Please see `01-orbit-organized.rs` for how couture makes this cleaner and neater.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 01-orbit-disorganized -- --help` outputs:
//!
//! ```terminal
//! Deploy and manage your apps from the terminal
//!
//! Usage: 01-orbit-disorganized <COMMAND>
//!
//! Commands:
//!   deploy    Ship the current project
//!   login     Sign in to your account
//!   logs      Tail live logs
//!   rollback  Roll back to a previous release
//!   search    Search your deployment history
//!   secrets   Manage environment secrets
//!   status    Show recent activity
//!   sync      Sync settings across machines
//!   help      Print this message or the help of the given subcommand(s)
//!
//! Options:
//!   -h, --help  Print help
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, Subcommand};

/// Deploy and manage your apps from the terminal.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Ship the current project.
    Deploy,
    /// Sign in to your account.
    Login,
    /// Tail live logs.
    Logs,
    /// Roll back to a previous release.
    Rollback,
    /// Search your deployment history.
    Search,
    /// Manage environment secrets.
    Secrets,
    /// Show recent activity.
    Status,
    /// Sync settings across machines.
    Sync,
}

fn main() {
    let cli = Cli::parse();
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
