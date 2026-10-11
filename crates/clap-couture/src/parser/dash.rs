//! `prompt(dash)` and `stdin(dash)`: `-` as an arg's value asks for it, or reads stdin.
//!
//! The `-` comes out of argv before anything else, so every later parse sees the arg left out,
//! and it is asked for or read like one the user never passed. An arg with both asks when it can,
//! at a terminal, and reads stdin otherwise, as when it is piped.

use std::ffi::{OsStr, OsString};

use clap::{ArgMatches, Command, error::ErrorKind, parser::ValueSource};

use super::{
    Failure,
    prompting::{arg_at, matches_at, raw_values, relax, unrequire, with_arg_at},
};
use crate::interactive::{PromptedArg, StdinArg};

/// An arg's subcommand path from the root, and its clap id.
type ArgAt = (Vec<&'static str>, &'static str);

/// The args the user passed `-` to, taken out of argv by [`take`].
pub(super) struct Dashed {
    /// Those to ask for.
    asked: Vec<ArgAt>,
    /// Those to read stdin into.
    read: Vec<ArgAt>,
}

impl Dashed {
    /// Check whether arg `id` of the command at `path` was passed `-` to be asked for.
    pub(super) fn asks(&self, path: &[&str], id: &str) -> bool {
        holds(&self.asked, path, id)
    }

    /// Check whether arg `id` of the command at `path` was passed `-` to read stdin.
    pub(super) fn reads(&self, path: &[&str], id: &str) -> bool {
        holds(&self.read, path, id)
    }

    /// `cmd` with no env var on any of these args.
    pub(super) fn without_env(&self, cmd: Command) -> Command {
        self.asked
            .iter()
            .chain(&self.read)
            .fold(cmd, |cmd, (path, id)| with_arg_at(cmd, path, id, |arg| arg.env(None)))
    }
}

/// `argv` without the `-` passed to each `prompt(dash)` arg in `prompted_args` and
/// `stdin(dash)` arg in `stdin_args`, and the args it was taken from. An arg with both is
/// asked for when `can_ask`, and reads stdin otherwise. `argv` that `cmd` rejects, eg. for
/// `--help`, is left to clap as is.
///
/// # Errors
/// A `-` that can't be taken out without changing another arg, as in `-vn-`.
pub(super) fn take(
    cmd: &Command,
    prompted_args: &[PromptedArg],
    stdin_args: &[StdinArg],
    argv: &[OsString],
    can_ask: bool,
) -> Result<(Vec<OsString>, Dashed), Failure> {
    let relaxed = stdin_args.iter().fold(relax(cmd.clone(), prompted_args), |cmd, stdin_arg| {
        unrequire(cmd, &stdin_arg.subcommand_path, stdin_arg.attr.arg_id)
    });
    let asking: Vec<ArgAt> = prompted_args
        .iter()
        .filter(|prompted_arg| prompted_arg.attr.dash)
        .map(|prompted_arg| (prompted_arg.subcommand_path.clone(), prompted_arg.attr.arg_id))
        .collect();
    let reading: Vec<ArgAt> = stdin_args
        .iter()
        .filter(|stdin_arg| stdin_arg.attr.dash)
        .map(|stdin_arg| (stdin_arg.subcommand_path.clone(), stdin_arg.attr.arg_id))
        .collect();
    let only_reading = reading.iter().filter(|(path, id)| !holds(&asking, path, id)).cloned();
    let dash_args: Vec<ArgAt> = asking.iter().cloned().chain(only_reading).collect();
    let mut argv = argv.to_vec();
    let mut dashed = Dashed { asked: Vec::new(), read: Vec::new() };
    for (path, id) in dash_args {
        let Ok(matches) = relaxed.clone().try_get_matches_from(&argv) else { break };
        let Some(level) = matches_at(&matches, &path) else { continue };
        if level.value_source(id) != Some(ValueSource::CommandLine)
            || raw_values(level, id) != ["-"]
        {
            continue;
        }
        let Some(rest) = without_dash(&relaxed, level, &path, id, &argv) else {
            return Err(error(
                cmd,
                &path,
                id,
                "got `-` in a way couture can't take out; pass `-` as a word of its own",
            ));
        };
        argv = rest;
        let asks = holds(&asking, &path, id) && (can_ask || !holds(&reading, &path, id));
        if asks { dashed.asked.push((path, id)) } else { dashed.read.push((path, id)) }
    }
    Ok((argv, dashed))
}

/// Check whether `args` holds arg `id` of the command at `path`.
fn holds(args: &[ArgAt], path: &[&str], id: &str) -> bool {
    args.iter().any(|(held_path, held_id)| held_path == path && *held_id == id)
}

/// The error for a `-` that asks for arg `id` of the command at `path` with no terminal to ask on.
pub(super) fn unaskable(cmd: &Command, path: &[&str], id: &str) -> Failure {
    error(cmd, path, id, "got `-` to be asked for, but there is no terminal to ask on")
}

/// `message` about arg `id` of the command at `path`, as clap formats its own errors.
fn error(cmd: &Command, path: &[&str], id: &str, message: &str) -> Failure {
    let mut built = cmd.clone();
    built.build();
    let arg = arg_at(&built, path, id).map_or_else(|| id.to_owned(), ToString::to_string);
    Failure::Clap(
        clap::Error::raw(ErrorKind::ValueValidation, format!("{arg} {message}"))
            .format(&mut cmd.clone()),
    )
}

/// `argv` with arg `id`'s `-` cut out, by one of [`cuts`]. `None` when no cut leaves the arg
/// unpassed and every other arg of its command, `level`, as it was.
fn without_dash(
    relaxed: &Command,
    level: &ArgMatches,
    path: &[&str],
    id: &str,
    argv: &[OsString],
) -> Option<Vec<OsString>> {
    let others = |level: &ArgMatches| {
        let mut others: Vec<_> = level
            .ids()
            // A group holds its args' values, so the one holding `id` would hold the `-` too.
            .filter(|other| other.as_str() != id && arg_at(relaxed, path, other.as_str()).is_some())
            .map(|other| {
                let values = raw_values(level, other.as_str());
                (other.clone(), level.value_source(other.as_str()), values)
            })
            .collect();
        others.sort_by(|left, right| left.0.cmp(&right.0));
        others
    };
    let before = others(level);
    // The first word is the binary's name.
    (1..argv.len()).flat_map(|index| cuts(argv, index)).find(|rest| {
        let Ok(matches) = relaxed.clone().try_get_matches_from(rest) else { return false };
        matches_at(&matches, path).is_some_and(|after| {
            after.value_source(id) != Some(ValueSource::CommandLine) && others(after) == before
        })
    })
}

/// The ways to cut a `-` ending word `index` out of `argv`: the word alone (`-`, `--flag=-`,
/// `-f-`), with the flag before it (`--flag -`), or with the last flag of a short cluster
/// (`-vf -`, `-vf-`, `-vf=-`). Empty when the word doesn't end in `-`.
fn cuts(argv: &[OsString], index: usize) -> Vec<Vec<OsString>> {
    let Some(word) = argv.get(index).filter(|word| word.as_encoded_bytes().ends_with(b"-")) else {
        return Vec::new();
    };
    let splice = |first: usize, replacement: Option<OsString>| {
        let mut rest = argv.to_vec();
        rest.splice(first..=index, replacement);
        rest
    };
    let previous = index.checked_sub(1).filter(|previous| *previous > 0);
    let mut cuts = vec![splice(index, None)];
    if let Some(previous) = previous {
        cuts.push(splice(previous, None));
        if word == "-"
            && let Some(head) = argv.get(previous).and_then(|flags| cluster_head(flags, ""))
        {
            cuts.push(splice(previous, Some(head)));
        }
    }
    cuts.extend(
        ["-", "=-"]
            .into_iter()
            .filter_map(|tail| cluster_head(word, tail))
            .map(|head| splice(index, Some(head))),
    );
    cuts
}

/// `word`, a cluster of short flags followed by `tail`, without its last flag and `tail`: `-vf-`
/// is `-v` for a `tail` of `-`. `None` for any other word, or a cluster of one flag.
fn cluster_head(word: &OsStr, tail: &str) -> Option<OsString> {
    let flags = word.to_str()?.strip_prefix('-')?.strip_suffix(tail)?;
    if flags.starts_with('-') {
        return None;
    }
    let (last, _) = flags.char_indices().last()?;
    flags.get(..last).filter(|head| !head.is_empty()).map(|head| OsString::from(format!("-{head}")))
}
