//! couture's `--help` keeps everything clap shows besides the subcommand listing.

use clap::{CommandFactory, Parser, Subcommand};
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
#[command(name = "copy", args_conflicts_with_subcommands = true)]
struct Conflicting {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Where to copy from.
    source: Option<String>,
}

#[derive(Parser, Couture)]
#[command(name = "copy", subcommand_negates_reqs = true)]
struct Negating {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Where to copy from.
    source: String,
}

#[derive(Parser, Couture)]
#[command(name = "copy", disable_help_subcommand = true)]
struct WithoutHelpSubcommand {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Couture)]
#[command(name = "copy")]
struct Uncategorized {
    #[command(subcommand)]
    cmd: UncategorizedCmd,
}

#[derive(Subcommand, Couture)]
enum UncategorizedCmd {
    /// Show what would be copied.
    Plan,
    /// Copy for real.
    Run,
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

// Its last arg has no help, so `--help` leaves that arg's help indent above the after-help.
#[derive(Parser, Couture)]
#[command(name = "copy", after_help = "See the manual.")]
struct Undocumented {
    /// Overwrite what is there.
    ///
    /// Even read-only files.
    #[arg(long)]
    force: bool,

    #[arg(long, help_heading = "Output")]
    into: Option<String>,
}

/// The help a command prints, as couture renders it and as clap does.
struct Helps {
    clap: String,
    couture: String,
}

impl Helps {
    fn of<T>() -> Self
    where
        T: CommandFactory + Couture,
    {
        let clap = T::command().render_help().to_string();
        let couture = T::command().with_couture::<T>().render_help().to_string();
        Self { clap, couture }
    }

    /// The usage of `help`: the `Usage:` line and any lines continuing it.
    fn usage(help: &str) -> &str {
        let mut paragraphs = help.split("\n\n").map(str::trim);
        paragraphs.find(|paragraph| paragraph.starts_with("Usage:")).unwrap_or_default()
    }
}

#[rstest]
#[case::one_usage(Helps::of::<Copy>())]
#[case::args_conflict_with_subcommands(Helps::of::<Conflicting>())]
#[case::subcommand_negates_reqs(Helps::of::<Negating>())]
#[case::without_subcommands(Helps::of::<German>())]
fn the_usage_is_clap_s(#[case] helps: Helps) {
    let (clap, couture) = (Helps::usage(&helps.clap), Helps::usage(&helps.couture));
    assert!(!clap.is_empty(), "{}", helps.clap);
    assert_eq!(couture, clap, "{}", helps.couture);
}

#[rstest]
fn clap_help_subcommand_is_listed_last() {
    let help = Copy::couture_command().render_help().to_string();
    let menu = "Everyday:\n  plan  Show what would be copied\n\n  help  Print this message or the \
                help of the given subcommand(s)\n";
    assert!(help.contains(menu), "{help}");
}

#[rstest]
fn an_uncategorized_menu_lists_what_clap_does() {
    let helps = Helps::of::<Uncategorized>();
    let commands = helps.clap.split("Commands:\n").nth(1).unwrap_or_default();
    let (commands, _) = commands.split_once("\n\n").unwrap_or_default();
    assert!(commands.ends_with("Print this message or the help of the given subcommand(s)"));
    assert!(helps.couture.contains(&format!("\n\n{commands}\n\n")), "{}", helps.couture);
}

#[rstest]
#[case::disabled(WithoutHelpSubcommand::couture_command().render_help().to_string())]
#[case::without_subcommands(German::couture_command().render_help().to_string())]
fn clap_help_subcommand_is_listed_only_when_clap_adds_it(#[case] help: String) {
    assert!(!help.contains("Print this message"), "{help}");
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

#[cfg(feature = "markdown")]
/// Copy **files**.
///
/// Reads _every_ source first.
#[derive(Parser, Couture)]
struct Described {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Overwrite **what** is there.
    ///
    /// Even _read-only_ files.
    #[arg(long)]
    force: bool,

    /// Copy into <DEST>.
    #[arg(long)]
    into: Option<String>,
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
fn an_arg_without_help_keeps_clap_s_indent_above_the_after_help() {
    let help = |cmd: clap::Command| {
        let err = cmd.try_get_matches_from(["copy", "--help"]).err();
        err.map(|err| err.render()).unwrap_or_default()
    };
    let clap = help(Undocumented::command());
    assert!(clap.to_string().contains("--into <INTO>\n          \n\nSee the manual."), "{clap}");
    assert_eq!(help(Undocumented::couture_command()).ansi().to_string(), clap.ansi().to_string());
}

#[rstest]
fn long_help_shows_the_long_text_and_short_help_the_short() {
    let mut cmd = Long::couture_command();
    let (short, long) = (cmd.render_help().to_string(), cmd.render_long_help().to_string());
    assert!(!short.contains("Read in full"), "{short}");
    assert!(long.contains("Read in full before anything is written."), "{long}");
    assert!(long.contains("Everyday:\n  plan"), "{long}");
}

#[cfg(feature = "markdown")]
#[rstest]
fn short_help_renders_markdown_in_the_about_and_arg_help() {
    let help = Described::couture_command().render_help().to_string();
    assert!(help.starts_with("Copy files\n"), "{help}");
    assert!(help.contains("--force        Overwrite what is there\n"), "{help}");
    assert!(!help.contains("**") && !help.contains("_every_"), "{help}");
}

#[cfg(feature = "markdown")]
#[rstest]
fn long_help_renders_markdown_in_the_long_about_and_arg_help() {
    let help = Described::couture_command().render_long_help().to_string();
    assert!(help.starts_with("Copy files.\n\nReads every source first.\n"), "{help}");
    // clap indents the blank line between the paragraphs too.
    assert!(
        help.contains("Overwrite what is there.\n          \n          Even read-only files."),
        "{help}"
    );
    assert!(!help.contains("**") && !help.contains('_'), "{help}");
}

#[cfg(feature = "markdown")]
#[rstest]
fn markdown_is_styled_on_a_terminal() {
    let help = Described::couture_command().render_help().ansi().to_string();
    assert!(help.contains("\u{1b}[1mfiles\u{1b}[0m"), "{help}");
    assert!(help.contains("\u{1b}[1mwhat\u{1b}[0m"), "{help}");
}

#[cfg(feature = "markdown")]
#[rstest]
fn an_angle_bracketed_placeholder_is_kept() {
    let help = Described::couture_command().render_help().to_string();
    assert!(help.contains("Copy into <DEST>\n"), "{help}");
}
