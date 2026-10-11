//! `#[couture(crate = ...)]` names clap-couture when a dependency renames it.

use clap::{Parser, Subcommand};
use couture::{Couture as _, CoutureParser as _};

#[derive(Parser, couture::Couture)]
#[couture(crate = couture, messages = { usage = "Verwendung:" })]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    #[arg(long)]
    #[couture(prompt)]
    name: Option<String>,
}

#[derive(Subcommand, couture::Couture)]
#[couture(crate = "couture", categories = { "everyday" = { title = "Everyday:" } })]
enum Cmd {
    /// Show recent activity.
    #[couture(category = "everyday")]
    Status,
}

#[test]
fn a_renamed_crate_derives() {
    let help = Cli::couture_command().render_help().to_string();
    assert!(
        help.contains("Verwendung:") && help.contains("Everyday:\n  status  Show recent activity"),
        "{help}"
    );
    assert_eq!(Cmd::CATEGORIES.find("status").map(couture::Category::label), Some("everyday"));
}
