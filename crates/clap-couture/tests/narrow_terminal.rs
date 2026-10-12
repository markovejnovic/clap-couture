//! In a terminal narrower than one of clap's lines, couture's help reads as clap's does: clap wraps
//! couture's text again, as an after-help, and must leave each row and its about whole.

use clap::{Command, CommandFactory as _, Parser, Subcommand};
use clap_couture::{CommandExt as _, Couture, CoutureParser as _, ParseError};
use rstest::rstest;
use unicode_width::UnicodeWidthStr as _;

/// The category description [`Orbit`] shows whole.
const DESCRIPTION: &str = "Ship code to production and back";

/// The texts each help must show whole, each line of one starting at the column its first does.
const TEXTS: [&str; 3] = [
    "Does a thing with many words in it",
    "Short one with several words [alias: s]",
    "Set the very long option to something useful",
];

/// The widths swept.
const WIDTHS: core::ops::RangeInclusive<usize> = 10..=80;

#[derive(Parser, Couture)]
#[command(name = "orbit")]
struct Orbit {
    #[command(subcommand)]
    cmd: Cmd,

    /// Set the very long option to something useful.
    #[arg(long, value_name = "VALUE")]
    some_very_long_option_name: Option<String>,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    "deploy" = { title = "Deploy:", description = "Ship code to production and back" }
})]
enum Cmd {
    /// Does a thing with many words in it.
    #[couture(category = "deploy")]
    ASubcommandWithAVeryLongNameXy,
    /// Short one with several words.
    #[command(visible_alias = "s")]
    Short,
}

// As `Orbit`, with no categories.
#[derive(Parser, Couture)]
#[command(name = "orbit")]
struct Uncategorized {
    #[command(subcommand)]
    cmd: UncategorizedCmd,

    /// Set the very long option to something useful.
    #[arg(long, value_name = "VALUE")]
    some_very_long_option_name: Option<String>,
}

#[derive(Subcommand, Couture)]
enum UncategorizedCmd {
    /// Does a thing with many words in it.
    ASubcommandWithAVeryLongNameXy,
    /// Short one with several words.
    #[command(visible_alias = "s")]
    Short,
}

// As `Orbit`, in the 20 columns it was seen to break in.
#[derive(Parser, Couture)]
#[command(name = "orbit", term_width = 20)]
struct Narrow {
    #[command(subcommand)]
    cmd: Cmd,

    /// Set the very long option to something useful.
    #[arg(long, value_name = "VALUE")]
    some_very_long_option_name: Option<String>,
}

/// `help` with clap's `Commands:` heading left out, as couture leaves it out.
fn without_commands_heading(help: &str) -> String {
    help.replace("\nCommands:\n", "\n")
}

/// `text` as `help` shows it: the words from the line `text` starts on, then from each line
/// below that starts at the same column; `None` when no line starts it.
fn shown(help: &str, text: &str) -> Option<String> {
    let first = text.split(' ').next()?;
    let lines: Vec<&str> = help.lines().collect();
    let (start, col) = lines.iter().enumerate().find_map(|(index, line)| {
        let col = line.match_indices(first).map(|(col, _)| col).find(|&col| {
            let rest = line.get(col..).unwrap_or_default();
            col > 0
                && line.get(..col).is_some_and(|before| before.ends_with(' '))
                && text.starts_with(rest.trim_end())
        })?;
        Some((index, col))
    })?;
    let indent = " ".repeat(col);
    let below = lines.iter().skip(start.saturating_add(1)).map_while(|line| {
        line.strip_prefix(&indent).filter(|rest| !rest.is_empty() && !rest.starts_with(' '))
    });
    let words = lines.get(start).and_then(|line| line.get(col..)).into_iter().chain(below);
    Some(words.flat_map(str::split_whitespace).collect::<Vec<_>>().join(" "))
}

/// Check that `help` shows each of `texts` whole and has no line of only spaces.
fn assert_readable(help: &str, width: usize, texts: &[&str]) {
    for &text in texts {
        assert_eq!(shown(help, text).as_deref(), Some(text), "at width {width}:\n{help}");
    }
    assert!(
        help.lines().all(|line| line.is_empty() || !line.trim().is_empty()),
        "at width {width}:\n{help}"
    );
}

/// `cmd`'s `-h` and `--help`, with couture's help installed for `T`.
fn helps<T>(cmd: Command) -> [String; 2]
where
    T: Couture,
{
    let mut cmd = cmd.with_couture::<T>();
    [cmd.render_help().to_string(), cmd.render_long_help().to_string()]
}

#[rstest]
fn every_row_stays_whole_at_every_width() {
    for width in WIDTHS {
        for help in helps::<Orbit>(Orbit::command().term_width(width)) {
            assert_readable(&help, width, &[DESCRIPTION]);
            assert_readable(&help, width, &TEXTS);
        }
        for help in helps::<Uncategorized>(Uncategorized::command().term_width(width)) {
            assert_readable(&help, width, &TEXTS);
        }
    }
}

#[rstest]
fn the_help_is_clap_s_while_clap_s_lines_fit() {
    let mut compared = 0usize;
    for width in WIDTHS {
        let mut clap = Uncategorized::command().term_width(width);
        let clap = clap.render_help().to_string();
        if clap.lines().any(|line| line.width() > width) {
            continue;
        }
        let [ours, _] = helps::<Uncategorized>(Uncategorized::command().term_width(width));
        assert_eq!(ours, without_commands_heading(&clap), "at width {width}");
        compared = compared.saturating_add(1);
    }
    assert!(compared > 20, "only {compared} widths were compared");
}

#[rstest]
#[case::root(&["--help"])]
#[case::root_short(&["-h"])]
#[case::nested(&["short", "--help"])]
fn a_narrow_parse_prints_couture_command_s_help(#[case] args: &[&str]) {
    let argv: Vec<&str> = core::iter::once("orbit").chain(args.iter().copied()).collect();
    let printed = match Narrow::couture_try_parse_from(&argv) {
        Err(ParseError::Clap(err)) => err.render().to_string(),
        other => format!("{:?}", other.err()),
    };
    let expected = Narrow::couture_command().try_get_matches_from(&argv).err();
    assert_eq!(printed, expected.map(|err| err.render().to_string()).unwrap_or_default());
    if args.first() != Some(&"short") {
        assert_readable(&printed, 20, &TEXTS);
        assert_readable(&printed, 20, &[DESCRIPTION]);
    }
}
