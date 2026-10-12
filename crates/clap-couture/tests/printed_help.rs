//! The help a parse prints is the help `couture_command` renders, though a parse renders only the
//! one it prints.

#[cfg(feature = "interactive")]
#[expect(dead_code, reason = "nothing is asked: help exits first")]
mod common;

use clap::{Args, Parser, Subcommand, error::ErrorKind};
#[cfg(feature = "interactive")]
use clap_couture::interactive::secrecy::SecretString;
use clap_couture::{Couture, CoutureParser as _, ParseError};
#[cfg(feature = "interactive")]
use common::ScriptedBackend;
use rstest::rstest;

/// Ship **it**.
#[cfg(feature = "interactive")]
#[derive(Parser, Couture)]
#[command(name = "deploy")]
struct Deploy {
    /// The app to ship.
    #[arg(long)]
    #[couture(prompt = "Which app?")]
    app: String,

    #[command(subcommand)]
    cmd: DeployCmd,
}

#[cfg(feature = "interactive")]
#[derive(Subcommand, Couture)]
#[couture(categories = { "ship" = { title = "Ship:" } })]
enum DeployCmd {
    /// Ship to **production**.
    #[couture(category = "ship")]
    Prod(Prod),
}

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
#[couture(categories = { "daily" = { title = "Daily:", description = "What you run *most*" } })]
enum GitCmd {
    /// Manage **remotes**.
    #[couture(category = "daily")]
    Remote(Remote),
    /// Show the working tree status.
    #[couture(category = "daily")]
    Status(Status),
    /// Manage tags.
    #[command(disable_help_subcommand = true)]
    Tag {
        #[command(subcommand)]
        cmd: TagCmd,
    },
    /// Listed by a template of its own.
    #[command(help_template = "{about}\n{subcommands}\n{after-help}")]
    Templated {
        #[command(subcommand)]
        cmd: TagCmd,
    },
}

/// What a failed parse prints.
#[derive(Debug, PartialEq, Eq)]
struct Printed {
    kind: Option<ErrorKind>,
    styled: String,
}

#[cfg(feature = "interactive")]
#[derive(Args, Couture)]
struct Prod {
    /// The token to ship with.
    #[arg(long, env = "PRINTED_HELP_TOKEN")]
    #[couture(prompt = "Which token?", secret)]
    token: SecretString,

    /// Ship anyway.
    #[arg(long)]
    #[couture(confirm = "Ship to production?")]
    yes: bool,
}

#[derive(Args, Couture)]
#[couture(messages = { usage = "Verwendung:", options = "Optionen:" })]
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
    Fetch,
}

#[derive(Args)]
struct Status {
    /// Show it short.
    ///
    /// One line per file, as `git status -s` does.
    #[arg(long)]
    short: bool,
}

#[derive(Subcommand, Couture)]
enum TagCmd {
    /// List the **tags**.
    List,
}

impl Printed {
    fn argv<'arg>(program: &'arg str, args: &[&'arg str]) -> Vec<&'arg str> {
        core::iter::once(program).chain(args.iter().copied()).collect()
    }

    /// What `T::couture_command` prints for `argv`.
    fn by_couture_command<T>(argv: &[&str]) -> Self
    where
        T: Parser + Couture,
    {
        let err = T::couture_command().try_get_matches_from(argv).err();
        Self { kind: err.as_ref().map(clap::Error::kind), styled: Self::styled(err) }
    }

    /// What a parse ending in `parsed` prints.
    fn by_parse<T>(parsed: Result<T, ParseError>) -> Self {
        match parsed {
            Err(ParseError::Clap(err)) => {
                Self { kind: Some(err.kind()), styled: Self::styled(Some(err)) }
            }
            other => Self { kind: None, styled: format!("{:?}", other.err()) },
        }
    }

    fn styled(err: Option<clap::Error>) -> String {
        err.map(|err| err.render().ansi().to_string()).unwrap_or_default()
    }
}

#[rstest]
#[case::root(&["--help"])]
#[case::root_short(&["-h"])]
#[case::root_missing_subcommand(&[])]
#[case::nested(&["remote", "--help"])]
#[case::nested_short(&["remote", "-h"])]
#[case::nested_missing_subcommand(&["remote"])]
#[case::two_levels_down(&["remote", "fetch", "--help"])]
#[case::global_arg_before(&["--verbose", "remote", "add", "-h"])]
#[case::long_help(&["status", "--help"])]
#[case::short_of_long_help(&["status", "-h"])]
#[case::help_subcommand(&["help"])]
#[case::help_subcommand_nested(&["help", "remote", "fetch"])]
#[case::nested_help_subcommand(&["remote", "help", "add"])]
#[case::help_of_help(&["help", "help"])]
#[case::without_help_subcommand(&["tag", "--help"])]
#[case::own_template(&["templated", "--help"])]
#[case::not_help(&["remote", "nope"])]
fn a_parse_prints_couture_command_s_help(#[case] args: &[&str]) {
    let argv = Printed::argv("git", args);
    let printed = Printed::by_parse(Git::couture_try_parse_from(&argv));
    assert_eq!(printed, Printed::by_couture_command::<Git>(&argv), "{}", printed.styled);
}

#[cfg(feature = "interactive")]
#[rstest]
#[case::root(&["--help"])]
#[case::nested(&["prod", "--help"])]
#[case::after_a_prompted_arg(&["--app", "web", "prod", "-h"])]
#[case::help_subcommand(&["help", "prod"])]
fn a_prompting_parse_prints_couture_command_s_help(
    #[case] args: &[&str],
    #[values(true, false)] available: bool,
) {
    let argv = Printed::argv("deploy", args);
    let backend = if available { ScriptedBackend::new([]) } else { ScriptedBackend::unavailable() };
    let printed = Printed::by_parse(Deploy::couture_try_parse_from_with(&backend, &argv));
    assert!(printed.kind.is_some(), "{}", printed.styled);
    assert_eq!(printed, Printed::by_couture_command::<Deploy>(&argv), "{}", printed.styled);
}
