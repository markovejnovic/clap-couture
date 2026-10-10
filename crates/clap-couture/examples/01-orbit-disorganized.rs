//! The `orbit` demo CLI on plain clap: one flat list of subcommands.
//! `01-orbit-organized` is the same CLI with couture.
//!
//! ```sh
//! cargo run -p clap-couture --example 01-orbit-disorganized -- --help
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "orbit",
    bin_name = "orbit",
    version,
    about = "Deploy and manage your apps from the terminal.",
    arg_required_else_help = true
)]
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
