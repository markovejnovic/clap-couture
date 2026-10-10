//! The demo CLI behind the README images.
//!
//! ```sh
//! cargo run -p clap-couture --example orbit --features markdown -- --help
//! ```

#![allow(
    clippy::expect_used,
    clippy::print_stdout,
    reason = "a runnable demo prints its result and uses `expect` for brevity"
)]

use clap::{
    CommandFactory as _, Parser, Subcommand,
    builder::styling::{Color, RgbColor, Style, Styles},
};
use clap_couture::{Couture, CoutureParser as _};

// Couture reuses the command's `Styles`: headings take `header`, command names take `literal`.
const PINK: Color = Color::Rgb(RgbColor(0xf5, 0xc2, 0xe7));
const BLUE: Color = Color::Rgb(RgbColor(0x89, 0xb4, 0xfa));
const GREY: Color = Color::Rgb(RgbColor(0x93, 0x99, 0xb2));

const STYLES: Styles = Styles::styled()
    .header(Style::new().fg_color(Some(PINK)).bold())
    .usage(Style::new().fg_color(Some(PINK)).bold())
    .literal(Style::new().fg_color(Some(BLUE)).bold())
    .placeholder(Style::new().fg_color(Some(GREY)));

#[derive(Parser, Couture)]
#[command(
    name = "orbit",
    version,
    about = "Deploy and manage your apps from the terminal.",
    styles = STYLES,
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
    // `ORBIT_PLAIN=1` prints plain clap help, for the "before" image in `assets/render.sh`.
    if std::env::var_os("ORBIT_PLAIN").is_some() {
        Cli::command().print_help().expect("write help");
        return;
    }

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
