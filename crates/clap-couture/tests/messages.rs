//! `#[couture(messages = { ... })]` replaces the text couture prints itself.

use clap::{CommandFactory as _, Parser, Subcommand};
use clap_couture::{CommandExt as _, Couture, CoutureParser as _};
use rstest::rstest;

#[derive(Parser, Couture)]
struct Default {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Couture)]
#[couture(messages = { usage_heading = "Verwendung", options_heading = "Optionen" })]
struct German {
    #[command(subcommand)]
    cmd: Cmd,
}

/// A later `#[couture(...)]` wins, but only for the messages it sets.
#[derive(Parser, Couture)]
#[couture(messages = { usage_heading = "Verwendung", options_heading = "Optionen" })]
#[couture(messages = { options_heading = String::from("Einstellungen") })]
struct Overridden {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(
    categories = { "everyday" = { title = "Alltag" } },
    messages = { options_heading = "Optionen" },
)]
enum Cmd {
    /// Tail live logs.
    #[category("everyday")]
    Logs,
}

#[rstest]
#[case::default(Default::couture_command(), "Usage:", "Options:")]
#[case::set(German::couture_command(), "Verwendung:", "Optionen:")]
#[case::set_twice(Overridden::couture_command(), "Verwendung:", "Einstellungen:")]
#[case::on_the_subcommand_enum(Default::command().with_couture::<Cmd>(), "Usage:", "Optionen:")]
fn help_headings_take_the_messages(
    #[case] mut cmd: clap::Command,
    #[case] usage_heading: &str,
    #[case] options_heading: &str,
) {
    let help = cmd.render_help().to_string();
    assert!(help.contains(usage_heading), "no `{usage_heading}` in:\n{help}");
    assert!(help.contains(options_heading), "no `{options_heading}` in:\n{help}");
}
