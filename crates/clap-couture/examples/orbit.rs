//! A runnable showcase of clap-couture's categorized help.
//!
//! ```sh
//! cargo run -p clap-couture --example orbit --features markdown -- --help
//! ```
//!
//! Or just `cargo run -p clap-couture --example orbit` — with no subcommand it
//! prints the couture help. This is also what `assets/demo.tape` records.

use clap::builder::styling::{Color, RgbColor, Style, Styles};
use clap::{CommandFactory as _, Parser, Subcommand};
use clap_couture::Couture;

// clap-couture inherits whatever `Styles` your command already uses, so a few
// colors here flow straight into the categorized help — category titles pick up
// the header style, command names pick up the literal style.
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
    // Demo affordance: `ORBIT_PLAIN=1` prints clap's default (ungrouped) help, so
    // `assets/render.sh` can capture the "before" image from this same definition.
    if std::env::var_os("ORBIT_PLAIN").is_some() {
        Cli::command().print_help().expect("write help");
        return;
    }

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
