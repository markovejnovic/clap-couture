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
#[couture(messages = { usage_heading = "Verwendung:", options_heading = "Optionen:" })]
struct German {
    #[command(subcommand)]
    cmd: Cmd,
}

/// A heading message carries its own colon, spaced the French way here.
#[derive(Parser, Couture)]
#[couture(messages = { usage_heading = "Utilisation :", options_heading = "Options :" })]
struct French {
    #[command(subcommand)]
    cmd: Cmd,
}

/// A later `#[couture(...)]` wins, but only for the messages it sets.
#[derive(Parser, Couture)]
#[couture(messages = { usage_heading = "Verwendung:", options_heading = "Optionen:" })]
#[couture(messages = { options_heading = String::from("Einstellungen:") })]
struct Overridden {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(
    categories = { "everyday" = { title = "Alltag:" } },
    messages = { options_heading = "Optionen:" },
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
#[case::own_punctuation(French::couture_command(), "Utilisation :", "Options :")]
#[case::on_the_subcommand_enum(Default::command().with_couture::<Cmd>(), "Usage:", "Optionen:")]
fn help_headings_take_the_messages(
    #[case] mut cmd: clap::Command,
    #[case] usage_heading: &str,
    #[case] options_heading: &str,
) {
    let help = cmd.render_help().to_string();
    for heading in [usage_heading, options_heading] {
        assert!(help.lines().any(|line| line == heading), "no `{heading}` line in:\n{help}");
    }
}
