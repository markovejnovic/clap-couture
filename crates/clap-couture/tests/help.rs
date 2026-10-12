//! couture's `--help` keeps everything clap shows besides the subcommand listing.

use clap::{CommandFactory as _, Parser, Subcommand};
use clap_couture::{CommandExt as _, Couture, CoutureParser as _};
use rstest::rstest;

#[derive(Parser, Couture)]
#[command(name = "copy", before_help = "Copies files.")]
struct Copy {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Overwrite what is there.
    #[arg(long)]
    force: bool,

    /// Where to copy from.
    source: String,

    /// Print each file copied.
    #[arg(long, help_heading = "Output")]
    verbose: bool,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "everyday" = { title = "Everyday:" } })]
enum Cmd {
    /// Show what would be copied.
    #[couture(category = "everyday")]
    Plan,
}

#[derive(Parser, Couture)]
#[couture(messages = { arguments = "Argumente:", options = "Optionen:" })]
struct German {
    /// Überschreiben.
    #[arg(long)]
    force: bool,

    /// Quelle.
    source: String,
}

#[derive(Parser, Couture)]
#[command(help_template = "{usage}\n{after-help}")]
struct Templated {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Couture)]
struct Long {
    #[command(subcommand)]
    cmd: Cmd,

    /// Where to copy from.
    ///
    /// Read in full before anything is written.
    #[arg(long)]
    source: String,
}

#[derive(Parser, Couture)]
struct Store {
    #[command(subcommand)]
    cmd: StoreCmd,
}

#[derive(Subcommand, Couture)]
enum StoreCmd {
    /// Delete <id> from the store.
    Delete,
}

#[rstest]
fn an_about_keeps_its_placeholders() {
    let help = Store::couture_command().render_help().to_string();
    assert!(help.contains("delete  Delete <id> from the store\n"), "{help}");
}

#[rstest]
fn positional_arguments_are_listed() {
    let help = Copy::couture_command().render_help().to_string();
    assert!(help.contains("Arguments:\n  <SOURCE>  Where to copy from\n"), "{help}");
}

#[rstest]
fn an_arg_with_a_help_heading_is_listed_under_it() {
    let help = Copy::couture_command().render_help().to_string();
    let options = help.split("Options:").nth(1).unwrap_or_default();
    let (options, output) = options.split_once("Output:").unwrap_or_default();
    assert!(!options.contains("--verbose"), "{help}");
    assert!(output.contains("--verbose  Print each file copied"), "{help}");
}

#[rstest]
fn sections_follow_clap_after_the_categories() {
    let help = Copy::couture_command().render_help().to_string();
    let order =
        ["Everyday:", "Arguments:", "Options:", "Output:"].map(|heading| help.find(heading));
    assert!(order.iter().all(Option::is_some) && order.is_sorted(), "{help}");
}

#[rstest]
fn a_before_help_is_kept() {
    let help = Copy::couture_command().render_help().to_string();
    assert!(help.starts_with("Copies files.\n\n"), "{help}");
}

#[rstest]
fn an_after_help_follows_the_args() {
    let help = Copy::command()
        .after_help("See the manual.")
        .with_couture::<Copy>()
        .render_help()
        .to_string();
    let ending = "Output:\n      --verbose  Print each file copied\n\nSee the manual.";
    assert!(help.trim_end().ends_with(ending), "{help}");
}

#[rstest]
fn the_default_headings_are_reworded() {
    let help = German::couture_command().render_help().to_string();
    assert!(help.contains("Argumente:\n  <SOURCE>"), "{help}");
    assert!(help.contains("Optionen:\n      --force"), "{help}");
    assert!(!help.contains("Arguments:") && !help.contains("Options:"), "{help}");
}

#[rstest]
fn a_help_template_is_kept() {
    let help = Templated::couture_command().render_help().to_string();
    assert!(help.starts_with("clap-couture <COMMAND>\n"), "{help}");
    assert!(help.contains("Everyday:\n  plan"), "{help}");
}

#[rstest]
fn long_help_shows_the_long_text_and_short_help_the_short() {
    let mut cmd = Long::couture_command();
    let (short, long) = (cmd.render_help().to_string(), cmd.render_long_help().to_string());
    assert!(!short.contains("Read in full"), "{short}");
    assert!(long.contains("Read in full before anything is written."), "{long}");
    assert!(long.contains("Everyday:\n  plan"), "{long}");
}
