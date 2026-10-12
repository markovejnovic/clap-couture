//! A nested subcommand's `--help` is couture's too: its categories, messages and markdown come
//! from the type that defines it, as the root's do.

use clap::{Args, Command, CommandFactory as _, Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};
use rstest::rstest;

/// The **stupid** content tracker.
#[derive(Parser, Couture)]
#[command(name = "git")]
struct Git {
    #[command(subcommand)]
    cmd: GitCmd,

    /// Say **more**.
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
    /// Manage **remotes**.
    Remote(Remote),
    /// Stash changes away.
    Stash {
        #[command(subcommand)]
        cmd: StashCmd,
    },
    /// Show the working tree status.
    Status,
    /// Manage tags.
    #[command(disable_help_subcommand = true)]
    Tag {
        #[command(subcommand)]
        cmd: TagCmd,
    },
}

#[derive(Subcommand, Couture)]
enum TagCmd {
    /// List the tags.
    List,
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
    /// The help plain clap prints.
    fn clap_text(&self) -> String {
        self.printed_by(Git::command())
    }

    /// The help `cmd` prints for these args.
    fn printed_by(&self, cmd: Command) -> String {
        let argv = core::iter::once(&"git").chain(self.0).chain(&["--help"]);
        cmd.try_get_matches_from(argv).err().map(|err| err.to_string()).unwrap_or_default()
    }

    fn text(&self) -> String {
        self.printed_by(Git::couture_command())
    }

    /// The `Usage:` line of `help`.
    fn usage_line(help: &str) -> Option<&str> {
        help.lines().find(|line| line.starts_with("Usage:"))
    }
}

#[rstest]
#[case::args_type(&["remote"], "Sync:\n  fetch  Fetch from a remote")]
#[case::inline_variant(&["branch"], "Local:\n  list  List the branches")]
#[case::two_levels_down(&["remote", "fetch"], "Everything:\n  all   Fetch every remote")]
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

#[cfg(feature = "markdown")]
#[rstest]
#[case::root(&[], "The stupid content tracker")]
#[case::nested(&["remote"], "Manage remotes")]
fn a_command_description_renders_markdown(
    #[case] args: &'static [&'static str],
    #[case] about: &str,
) {
    let help = HelpOf(args).text();
    assert!(help.starts_with(&format!("{about}\n")), "{help}");
    assert!(!help.contains("**"), "{help}");
}

#[cfg(feature = "markdown")]
#[rstest]
fn a_nested_arg_help_renders_markdown() {
    let help = HelpOf(&["remote", "fetch"]).text();
    assert!(help.contains("--verbose  Say more\n"), "{help}");
    assert!(!help.contains("**"), "{help}");
}

#[rstest]
#[case::categorized(&["remote"], "Sync:\n  fetch  Fetch from a remote\n\n  help   Print this")]
#[case::uncategorized(&["stash"], "  push  Stash the changes\n  help  Print this")]
fn a_nested_menu_lists_clap_help_subcommand_last(
    #[case] args: &'static [&'static str],
    #[case] ending: &str,
) {
    let help = HelpOf(args).text();
    let ending = format!("{ending} message or the help of the given subcommand(s)\n\n");
    assert!(help.contains(&ending), "{help}");
}

#[rstest]
#[case::disabled(&["tag"])]
#[case::without_subcommands(&["status"])]
fn a_nested_menu_lists_clap_help_subcommand_only_when_clap_adds_it(
    #[case] args: &'static [&'static str],
) {
    let help = HelpOf(args).text();
    assert!(!help.contains("Print this message"), "{help}");
}

#[rstest]
#[case::root(&[])]
#[case::nested(&["branch"])]
#[case::two_levels_down(&["branch", "list"])]
#[case::leaf(&["status"])]
fn a_nested_usage_is_clap_s(#[case] args: &'static [&'static str]) {
    let of = HelpOf(args);
    let (help, clap) = (of.text(), of.clap_text());
    assert!(HelpOf::usage_line(&clap).is_some(), "{clap}");
    assert_eq!(HelpOf::usage_line(&help), HelpOf::usage_line(&clap), "{help}");
}

#[rstest]
fn a_nested_help_lists_the_global_args() {
    let help = HelpOf(&["remote"]).text();
    assert!(help.contains("--verbose  Say "), "{help}");
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
    let usage_line = format!("{usage_heading} git");
    assert!(
        help.lines().any(|line| line.starts_with(&usage_line)),
        "no `{usage_line}` in:\n{help}"
    );
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
