//! What the prompting parse assumes of clap, checked against the clap actually built: which rules
//! make an arg required and which of them couture can drop, which args conflict, how a default
//! splits into values, and how clap words a value count it refuses.

use clap::{Arg, ArgAction, ArgGroup, Command, builder::ValueRange, error::ErrorKind};
use rstest::rstest;

use super::prompting::{BinNamed as _, ValueCount, blocked, default_values, summary, unrequire};
use crate::command_shapes::Choices;

/// The args of a generated conflict command, each a `--name <VALUE>` option.
const IDS: [&str; 5] = ["alpha", "beta", "gamma", "delta", "omega"];

/// How a generated command makes `--target` required.
#[derive(Clone, Copy, Debug)]
enum Requirement {
    /// As a member of a required group.
    Group,
    /// When `--other` is `on`.
    IfEq,
    /// As a member of a required group that takes several.
    MultipleGroup,
    Required,
    UnlessPresent,
    UnlessPresentAll,
    UnlessPresentAny,
}

impl Requirement {
    /// A command whose `--target` this rule requires, beside `--other` and `--third`.
    fn command(self) -> Command {
        let target = Arg::new("target").long("target");
        let target = match self {
            Self::Group | Self::MultipleGroup => target,
            Self::Required => target.required(true),
            Self::IfEq => target.required_if_eq("other", "on"),
            Self::UnlessPresent => target.required_unless_present("other"),
            Self::UnlessPresentAll => target.required_unless_present_all(["other", "third"]),
            Self::UnlessPresentAny => target.required_unless_present_any(["other", "third"]),
        };
        let cmd = Command::new("app")
            .arg(target)
            .arg(Arg::new("other").long("other"))
            .arg(Arg::new("third").long("third"));
        match self {
            Self::Group => {
                cmd.group(ArgGroup::new("pick").args(["target", "other"]).required(true))
            }
            Self::MultipleGroup => cmd.group(
                ArgGroup::new("pick").args(["target", "third"]).required(true).multiple(true),
            ),
            Self::IfEq
            | Self::Required
            | Self::UnlessPresent
            | Self::UnlessPresentAll
            | Self::UnlessPresentAny => cmd,
        }
    }

    /// Check whether couture's `unrequire` drops this rule; its doc names the ones it can't.
    const fn dropped_by_unrequire(self) -> bool {
        !matches!(self, Self::IfEq | Self::UnlessPresentAll)
    }
}

/// Check whether clap refuses `argv` as missing a required arg.
fn misses_required(cmd: &Command, argv: &[&str]) -> bool {
    cmd.clone()
        .try_get_matches_from(argv)
        .is_err_and(|err| err.kind() == ErrorKind::MissingRequiredArgument)
}

/// A command of options from [`IDS`], related by random conflicts, exclusivity and a group, and
/// the same command without the group's own conflict.
fn conflict_commands(choices: &mut Choices) -> (Command, Command) {
    let members: Vec<&str> = IDS.iter().copied().filter(|_| choices.one_in(2)).collect();
    let mut cmd = Command::new("app");
    for id in IDS {
        let mut arg = Arg::new(id).long(id).exclusive(choices.one_in(12));
        for other in IDS.iter().filter(|other| **other != id) {
            if choices.one_in(8) {
                arg = arg.conflicts_with(*other);
            }
        }
        // A member conflicting with its own group could never be passed.
        if !members.contains(&id) && choices.one_in(6) {
            arg = arg.conflicts_with("pair");
        }
        cmd = cmd.arg(arg);
    }
    let group = ArgGroup::new("pair").args(members.clone()).multiple(choices.one_in(2));
    let outsider = IDS.iter().find(|id| !members.contains(id)).filter(|_| choices.one_in(4));
    let without_group_conflict = cmd.clone().group(group.clone());
    let group = match outsider {
        Some(outsider) => group.conflicts_with(*outsider),
        None => group,
    };
    (cmd.group(group), without_group_conflict)
}

#[rstest]
fn an_arg_couture_asks_for_conflicts_where_clap_says_so() {
    let (mut checked, mut only_by_group_conflict) = (0usize, 0usize);
    for seed in 0u64..400u64 {
        let (cmd, without_group_conflict) = conflict_commands(&mut Choices::new(seed));
        let mut built = cmd.clone();
        built.build();
        for passed in IDS {
            let passed_flag = format!("--{passed}=1");
            let Ok(matches) = cmd.clone().try_get_matches_from(["app", &passed_flag]) else {
                continue;
            };
            for asked in IDS.iter().filter(|asked| **asked != passed) {
                let both = ["app", &passed_flag, &format!("--{asked}=2")];
                let refuses = |cmd: &Command| {
                    cmd.clone()
                        .try_get_matches_from(both)
                        .is_err_and(|err| err.kind() == ErrorKind::ArgumentConflict)
                };
                let clap_refuses = refuses(&cmd);
                only_by_group_conflict = only_by_group_conflict
                    .saturating_add(usize::from(refuses(&without_group_conflict) != clap_refuses));
                assert_eq!(
                    blocked(&built, &matches, &[], asked, &[]),
                    clap_refuses,
                    "asking --{asked} after --{passed}, for seed {seed}",
                );
                checked = checked.saturating_add(1);
            }
        }
    }
    assert!(checked > 5000, "only {checked} pairs were checked");
    assert!(only_by_group_conflict > 0, "no group conflict was generated");
}

#[rstest]
fn a_default_splits_into_the_values_clap_takes(
    #[values(&["3"][..], &["a,b"][..], &["a,b", "c"][..], &[""][..], &["a,,b"][..], &[",a,"][..])]
    defaults: &'static [&'static str],
    #[values(None, Some(','), Some(';'))] delimiter: Option<char>,
    #[values(ArgAction::Set, ArgAction::Append)] action: ArgAction,
) {
    let arg = Arg::new("v").long("v").action(action).default_values(defaults.iter().copied());
    let arg = match delimiter {
        Some(delimiter) => arg.value_delimiter(delimiter),
        None => arg.num_args(1..),
    };
    let cmd = Command::new("app").arg(arg);
    let Ok(matches) = cmd.clone().try_get_matches_from(["app"]) else {
        return;
    };
    let clap: Vec<String> = matches
        .get_raw("v")
        .into_iter()
        .flatten()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    let mut built = cmd;
    built.build();
    let arg = built.get_arguments().find(|arg| arg.get_id() == "v");
    let couture: Vec<&str> = arg.into_iter().flat_map(default_values).collect();
    assert_eq!(couture, clap);
}

#[rstest]
fn the_requirements_couture_drops_are_the_ones_clap_lets_it(
    #[values(
        Requirement::Group,
        Requirement::MultipleGroup,
        Requirement::Required,
        Requirement::IfEq,
        Requirement::UnlessPresent,
        Requirement::UnlessPresentAll,
        Requirement::UnlessPresentAny
    )]
    requirement: Requirement,
    #[values(&["app"][..], &["app", "--other=on"][..], &["app", "--third=on"][..])] argv: &[&str],
) {
    let cmd = requirement.command();
    let relaxed = unrequire(cmd.clone(), &[], "target");
    let (missing, still_missing) = (misses_required(&cmd, argv), misses_required(&relaxed, argv));
    if requirement.dropped_by_unrequire() {
        assert!(!still_missing, "{requirement:?} still requires --target on {argv:?}");
    } else {
        assert_eq!(still_missing, missing, "{requirement:?} on {argv:?}");
    }
    // Something requires `--target` somewhere in the corpus, so the drop is no vacuous pass.
    let requires_somewhere =
        [["app"].as_slice(), &["app", "--other=on"]].iter().any(|argv| misses_required(&cmd, argv));
    assert!(requires_somewhere, "{requirement:?} never requires --target");
}

#[rstest]
fn a_value_count_couture_refuses_is_worded_as_clap_words_it(
    #[values(
        ValueRange::new(1),
        ValueRange::new(2),
        ValueRange::new(3),
        ValueRange::new(0..=1),
        ValueRange::new(1..=2),
        ValueRange::new(1..=3),
        ValueRange::new(2..=3),
        ValueRange::new(2..=4),
        ValueRange::new(1..),
        ValueRange::new(2..),
        ValueRange::new(3..)
    )]
    range: ValueRange,
    #[values(ArgAction::Set, ArgAction::Append)] action: ArgAction,
    #[values(1, 2, 3, 4, 5)] count: usize,
) {
    let values: Vec<String> = (0..count).map(|index| format!("v{index}")).collect();
    // Past the most, typed values become positionals, so clap has no wording to compare with.
    if range.max_values() < count {
        return;
    }
    let arg = Arg::new("v").long("v").action(action).num_args(range);
    let cmd = Command::new("app").arg(arg);
    let argv = ["app", "--v"].into_iter().chain(values.iter().map(String::as_str));
    let clap = cmd.clone().try_get_matches_from(argv);
    let mut built = cmd;
    built.build();
    let Some(count_rule) =
        built.get_arguments().find(|arg| arg.get_id() == "v").and_then(ValueCount::of)
    else {
        return;
    };
    let couture = count_rule.check(&values).err();
    let clap = clap.err();
    let kinds = |err: &Option<clap::Error>| err.as_ref().map(clap::Error::kind);
    assert_eq!(kinds(&couture), kinds(&clap), "{range:?} with {count} values");
    let wording = |err: Option<clap::Error>| err.map(|err| summary(&err, &values));
    assert_eq!(wording(couture), wording(clap), "{range:?} with {count} values");
}

#[rstest]
fn couture_names_the_binary_as_clap_does(
    #[values(
        "tool",
        "/usr/bin/tool",
        "./tool",
        "../bin/tool.exe",
        "C:\\Tools\\tool.exe",
        "tool/",
        "/",
        ".",
        "..",
        "dir with spaces/my tool",
        "~/.local/bin/t\u{e9}l"
    )]
    called: &str,
    #[values(false, true)] no_binary_name: bool,
) {
    let cmd = Command::new("tool")
        .no_binary_name(no_binary_name)
        .arg(Arg::new("x").long("x").required(true));
    let argv: Vec<std::ffi::OsString> =
        if no_binary_name { Vec::new() } else { Vec::from([called.into()]) };
    let clap = cmd.clone().try_get_matches_from(&argv).err().map(|err| err.to_string());
    let usage_line =
        |text: &str| text.lines().find(|line| line.starts_with("Usage:")).map(str::to_owned);
    let clap = clap.as_deref().and_then(usage_line);
    assert!(clap.is_some(), "{called:?}");
    let couture = cmd.bin_named(&argv).render_usage().to_string();
    assert_eq!(usage_line(&couture), clap, "{called:?}");
}
