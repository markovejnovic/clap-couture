//! A runnable showcase of clap-couture's categorized help.
//!
//! ```sh
//! cargo run -p clap-couture --example orbit --features markdown -- --help
//! ```
//!
//! Or just `cargo run -p clap-couture --example orbit` — with no subcommand it
//! prints the couture help. This is also what `assets/demo.tape` records.

use clap::{Parser, Subcommand};
use clap_couture::Couture;

#[derive(Parser, Couture)]
#[command(
    name = "orbit",
    version,
    about = "Deploy and manage your apps from the terminal.",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    "everyday" = { title = "Everyday", description = "The commands you'll reach for daily" },
    "deploy"   = { title = "Deploy",   description = "Ship code to production" },
    "account"  = { title = "Account & sync" },
})]
enum Cmd {
    /// Search your deployment history
    #[category("everyday")]
    Search,
    /// Show recent activity
    #[category("everyday")]
    Status,
    /// Tail live logs
    #[category("everyday")]
    Logs,
    /// Ship the current project
    #[category("deploy")]
    Deploy,
    /// Roll back to a previous release
    #[category("deploy")]
    Rollback,
    /// Manage environment secrets
    #[category("deploy")]
    Secrets,
    /// Sign in to your account
    #[category("account")]
    Login,
    /// Sync settings across machines
    #[category("account")]
    Sync,
}

fn main() {
    let cli = Cli::couture_parse();
    // A real CLI would dispatch here; the demo just reports what it parsed.
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
