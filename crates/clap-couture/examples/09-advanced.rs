//! The `orbit` demo CLI with couture: subcommands grouped into titled categories.
//! `01-orbit-disorganized` is the same CLI on plain clap.
//!
//! ```sh
//! cargo run -p clap-couture --example 09-advanced -- --help
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};

/// Deploy and manage your apps from the terminal.
#[derive(Parser, Couture)]
#[command(name = "orbit", bin_name = "orbit", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    "everyday" = { title = "Everyday:", description = "The commands you'll reach for daily" },
    "deploy"   = { title = "Deploy:",  description = "Ship code to production" },
    "account"  = { title = "Account & sync:" },
})]
enum Cmd {
    /// Ship the current project.
    #[category("deploy")]
    Deploy,
    /// Sign in to your account.
    #[category("account")]
    Login,
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
    Secrets,
    /// Show recent activity.
    #[category("everyday")]
    Status,
    /// Sync settings across machines.
    #[category("account")]
    Sync,
}

fn main() {
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
