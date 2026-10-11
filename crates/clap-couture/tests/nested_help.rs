//! A nested subcommand's `--help` is couture's too: its categories, messages and markdown come
//! from the type that defines it, as the root's do.

use clap::{Args, Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};
use rstest::rstest;

#[derive(Parser, Couture)]
#[command(name = "git")]
struct Git {
    #[command(subcommand)]
    cmd: GitCmd,

    /// Say more.
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Couture)]
enum GitCmd {
    /// Manage branches.
    Branch {
        #[command(subcommand)]
        cmd: BranchCmd,
    },
    #[command(flatten)]
    Plumbing(Plumbing),
    /// Manage remotes.
    Remote(Remote),
    /// Stash changes away.
    Stash {
        #[command(subcommand)]
        cmd: StashCmd,
    },
    /// Show the working tree status.
    Status,
}

#[derive(Subcommand, Couture)]
#[couture(messages = { usage = "Utilisation :" })]
enum Plumbing {
    /// Print an object.
    CatFile,
}

/// Without the derive, its commands are listed uncategorized.
#[derive(Subcommand)]
enum StashCmd {
    /// Stash the changes.
    Push,
}

#[derive(Args, Couture)]
#[couture(messages = { usage = "Verwendung:" })]
struct Remote {
    #[command(subcommand)]
    cmd: RemoteCmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "sync" = { title = "Sync:" } })]
enum RemoteCmd {
    /// Add a **new** remote.
    Add,
    /// Fetch from a remote.
    #[couture(category = "sync")]
    Fetch(Fetch),
}

#[derive(Args, Couture)]
struct Fetch {
    #[command(subcommand)]
    cmd: FetchCmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "everything" = { title = "Everything:" } })]
enum FetchCmd {
    /// Fetch every remote.
    #[couture(category = "everything")]
    All,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "local" = { title = "Local:" } }, messages = { options = "Optionen:" })]
enum BranchCmd {
    /// List the branches.
    #[couture(category = "local")]
    List,
}

/// Run `git` with `args` and `--help`, and return the help it prints.
struct HelpOf(&'static [&'static str]);

impl HelpOf {
    fn text(&self) -> String {
        let argv = core::iter::once(&"git").chain(self.0).chain(&["--help"]);
        Git::couture_command()
            .try_get_matches_from(argv)
            .err()
            .map(|err| err.to_string())
            .unwrap_or_default()
    }
}

#[rstest]
#[case::args_type(&["remote"], "Sync:\n  fetch  Fetch from a remote")]
#[case::inline_variant(&["branch"], "Local:\n  list  List the branches")]
#[case::two_levels_down(&["remote", "fetch"], "Everything:\n  all  Fetch every remote")]
#[case::without_couture(&["stash"], "\n  push  Stash the changes\n")]
fn a_nested_menu_is_categorized(#[case] args: &'static [&'static str], #[case] section: &str) {
    let help = HelpOf(args).text();
    assert!(help.contains(section), "{help}");
    assert!(!help.contains("Commands:"), "{help}");
}

#[cfg(feature = "markdown")]
#[rstest]
fn a_nested_menu_renders_markdown() {
    let help = HelpOf(&["remote"]).text();
    assert!(help.contains("  add    Add a new remote"), "{help}");
    assert!(!help.contains("**"), "{help}");
}

#[rstest]
fn a_nested_menu_leaves_out_clap_help_subcommand() {
    let help = HelpOf(&["remote"]).text();
    assert!(!help.lines().any(|line| line.trim_start().starts_with("help")), "{help}");
}

#[rstest]
fn a_nested_help_lists_the_global_args() {
    let help = HelpOf(&["remote"]).text();
    assert!(help.contains("--verbose  Say more"), "{help}");
}

#[rstest]
#[case::root(&[], "Usage:")]
#[case::args_type(&["remote"], "Verwendung:")]
#[case::inherited(&["remote", "fetch"], "Verwendung:")]
#[case::sibling(&["status"], "Usage:")]
#[case::flattened_enum(&["cat-file"], "Utilisation :")]
fn a_nested_help_takes_its_type_messages(
    #[case] args: &'static [&'static str],
    #[case] usage_heading: &str,
) {
    let help = HelpOf(args).text();
    assert!(help.lines().any(|line| line == usage_heading), "no `{usage_heading}` in:\n{help}");
}

#[rstest]
#[case::the_command_holding_it(&["branch"], "Options:")]
#[case::its_variants(&["branch", "list"], "Optionen:")]
fn a_subcommand_enum_messages_word_its_variants(
    #[case] args: &'static [&'static str],
    #[case] options_heading: &str,
) {
    let help = HelpOf(args).text();
    assert!(help.lines().any(|line| line == options_heading), "no `{options_heading}` in:\n{help}");
}
