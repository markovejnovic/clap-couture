//! Parse, ask for the prompted args that got no value, then parse again with the answers.
//!
//! Every answer goes back into the command as its arg's `default_values`, so clap converts and
//! validates it exactly like a typed value. Each answer is checked with a trial parse as soon as
//! it is given: the previous trial passed, so a failure can only come from the new answer, which
//! is then asked again with clap's message. For an arg asked one value at a time, each value is
//! an answer of its own, checked on top of the values before it.

use core::iter;
use std::ffi::OsString;

use clap::{
    Arg, ArgAction, ArgGroup, ArgMatches, Command, FromArgMatches, Id, builder::PossibleValue,
    error::ErrorKind, parser::ValueSource,
};

use super::Failure;
use crate::{
    Messages,
    interactive::{
        Backend, CommandStyles, ConfirmPrompt, MultiSelectPrompt, PromptError, PromptedArg,
        PromptedArgTree, SelectPrompt, TextPrompt,
    },
};

/// A prompted arg's accepted answer.
struct Answer<'prompted_arg> {
    prompted_arg: &'prompted_arg PromptedArg,
    /// One value, or any number for an arg that takes many.
    values: Vec<String>,
}

/// One parse: the command, its argv, its prompted args and who to ask.
struct Session<'run> {
    argv: &'run [OsString],
    backend: &'run dyn Backend,
    cmd: &'run Command,
    messages: &'run Messages,
    prompted_args: &'run [PromptedArg],
}

impl Session<'_> {
    /// Ask for `prompted_arg` until clap accepts the answer; `None` when an optional single-value
    /// one is left empty. `current` is the arg's default, split into its values.
    fn answer(
        &self,
        prompted_arg: &PromptedArg,
        current: &[String],
        answers: &[Answer<'_>],
    ) -> Result<Option<Vec<String>>, PromptError> {
        let Some(arg) = arg_at(self.cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
        else {
            return Ok(None);
        };
        let question = prompted_arg.attr.question.as_ref().map_or_else(
            || {
                arg.get_help()
                    .map_or_else(|| prompted_arg.attr.arg_id.to_owned(), ToString::to_string)
            },
            |question| question.get().into_owned(),
        );
        let options: Vec<PossibleValue> =
            arg.get_possible_values().into_iter().filter(|value| !value.is_hide_set()).collect();
        if !takes_many(arg) {
            return self.until_accepted(answers, prompted_arg, || {
                Ok(self.ask(arg, &question, &options, current.first())?.map(|value| vec![value]))
            });
        }
        if options.is_empty() {
            return self.each_value(arg, &question, prompted_arg, current, answers).map(Some);
        }
        self.until_accepted(answers, prompted_arg, || {
            self.backend
                .multi_select(&MultiSelectPrompt {
                    defaults: current,
                    optional: !arg.is_required_set(),
                    options: &options,
                    question: &question,
                    styles: CommandStyles(self.cmd.get_styles()),
                })
                .map(Some)
        })
    }

    fn ask(
        &self,
        arg: &Arg,
        question: &str,
        options: &[PossibleValue],
        current: Option<&String>,
    ) -> Result<Option<String>, PromptError> {
        let styles = CommandStyles(self.cmd.get_styles());
        if let Some(negate) = flag_negation(arg.get_action()) {
            let set = self.backend.confirm(&ConfirmPrompt { default: false, question, styles })?;
            return Ok(Some((set != negate).to_string()));
        }
        if options.is_empty() {
            return self.backend.text(&TextPrompt {
                default: current.map(String::as_str),
                hint: None,
                initial: None,
                optional: !arg.is_required_set(),
                question,
                styles,
            });
        }
        self.backend.select(&SelectPrompt {
            default: current.map(String::as_str),
            none_option: &self.messages.none.get(),
            optional: !arg.is_required_set(),
            options,
            question,
            styles,
        })
    }

    /// Ask for one value at a time until an empty answer, each starting out as the default's
    /// value at its position. Only a required arg's first value may not be empty.
    ///
    /// The default goes in as initial text rather than as the answer Enter accepts, so clearing it
    /// still finishes the list.
    fn each_value(
        &self,
        arg: &Arg,
        question: &str,
        prompted_arg: &PromptedArg,
        current: &[String],
        answers: &[Answer<'_>],
    ) -> Result<Vec<String>, PromptError> {
        let finish_hint = self.messages.finish_hint.get();
        let mut values = Vec::new();
        loop {
            let optional = !values.is_empty() || !arg.is_required_set();
            let Some(value) = self.backend.text(&TextPrompt {
                default: None,
                hint: optional.then_some(&*finish_hint),
                initial: current.get(values.len()).map(String::as_str),
                optional,
                question,
                styles: CommandStyles(self.cmd.get_styles()),
            })?
            else {
                return Ok(values);
            };
            values.push(value);
            if let Err(err) = self.trial(answers, prompted_arg, &values) {
                values.pop();
                self.backend.report(&summary(&err), CommandStyles(self.cmd.get_styles()))?;
            }
        }
    }

    /// Parse with `candidate` as `prompted_arg`'s answer on top of the accepted ones.
    fn trial(
        &self,
        answers: &[Answer<'_>],
        prompted_arg: &PromptedArg,
        candidate: &[String],
    ) -> Result<(), clap::Error> {
        let accepted = answers.iter().map(|answer| (answer.prompted_arg, answer.values.as_slice()));
        let cmd = inject(
            relax(self.cmd.clone(), self.prompted_args),
            accepted.chain(iter::once((prompted_arg, candidate))),
        );
        cmd.try_get_matches_from(self.argv).map(drop)
    }

    /// Call `ask` until clap accepts its answer, showing clap's complaint before each retry.
    fn until_accepted(
        &self,
        answers: &[Answer<'_>],
        prompted_arg: &PromptedArg,
        mut ask: impl FnMut() -> Result<Option<Vec<String>>, PromptError>,
    ) -> Result<Option<Vec<String>>, PromptError> {
        loop {
            let Some(values) = ask()? else { return Ok(None) };
            match self.trial(answers, prompted_arg, &values) {
                Ok(()) => return Ok(Some(values)),
                Err(err) => {
                    self.backend.report(&summary(&err), CommandStyles(self.cmd.get_styles()))?;
                }
            }
        }
    }
}

/// Parse `argv` against `cmd`, asking `backend` for the prompted args in `prompted_arg_tree` that
/// got no value.
pub(super) fn parse<T>(
    cmd: Command,
    prompted_arg_tree: &'static PromptedArgTree,
    messages: &Messages,
    argv: &[OsString],
    backend: &dyn Backend,
) -> Result<T, Failure>
where
    T: FromArgMatches,
{
    let prompted_args = prompted_arg_tree.prompted_args();
    if prompted_args.is_empty() || !backend.is_available() {
        return super::parse(cmd, argv);
    }
    // Help, usage and errors come from the command the user sees, not the relaxed one.
    let Ok(matches) = relax(cmd.clone(), &prompted_args).try_get_matches_from(argv) else {
        return super::parse(cmd, argv);
    };

    let session = Session { argv, backend, cmd: &cmd, messages, prompted_args: &prompted_args };
    let mut answers = Vec::new();
    for (prompted_arg, current) in pending(&cmd, &matches, &prompted_args) {
        if blocked(&cmd, &matches, prompted_arg, &answers) {
            continue;
        }
        match session.answer(prompted_arg, &current, &answers) {
            Ok(Some(values)) => answers.push(Answer { prompted_arg, values }),
            Ok(None) => {}
            Err(PromptError::Cancelled) => return Err(Failure::Cancelled),
            Err(PromptError::Io(err)) => {
                return Err(Failure::Clap(clap::Error::raw(ErrorKind::Io, err)));
            }
            Err(PromptError::NotATerminal) => return super::parse(cmd, argv),
        }
    }
    let cmd_with_answers =
        inject(cmd, answers.iter().map(|answer| (answer.prompted_arg, answer.values.as_slice())));
    super::parse(cmd_with_answers, argv)
}

fn arg_at<'cmd>(cmd: &'cmd Command, path: &[&str], id: &str) -> Option<&'cmd Arg> {
    command_at(cmd, path)?.get_arguments().find(|arg| arg.get_id() == id)
}

/// Check whether `prompted_arg` conflicts with an arg the user passed or an earlier answer.
///
/// Answers go in as default values, which clap never checks for conflicts, so asking here
/// would let the parse land in a state clap rejects when typed.
fn blocked(
    cmd: &Command,
    matches: &ArgMatches,
    prompted_arg: &PromptedArg,
    answers: &[Answer<'_>],
) -> bool {
    let (Some(level), Some(level_matches)) = (
        command_at(cmd, &prompted_arg.subcommand_path),
        matches_at(matches, &prompted_arg.subcommand_path),
    ) else {
        return false;
    };
    let Some(arg) = level.get_arguments().find(|arg| arg.get_id() == prompted_arg.attr.arg_id)
    else {
        return false;
    };
    let answered = |id: &Id| {
        answers.iter().any(|answer| {
            answer.prompted_arg.subcommand_path == prompted_arg.subcommand_path
                && answer.prompted_arg.attr.arg_id == id
        })
    };
    let conflicts = level.get_arg_conflicts_with(arg);
    level
        .get_arguments()
        .filter(|other| other.get_id() != arg.get_id())
        .filter(|other| answered(other.get_id()) || explicit(level_matches, other.get_id()))
        .any(|other| {
            other.is_exclusive_set()
                || conflicts.iter().any(|conflict| conflict.get_id() == other.get_id())
                || level.get_arg_conflicts_with(other).iter().any(|c| c.get_id() == arg.get_id())
                || group_mates(level, arg).any(|mate| mate == other.get_id())
        })
}

/// `arg`'s declared default values, split at its value delimiter.
/// Without a delimiter, each default is one value. Defaults are set from strings, so any that
/// is not UTF-8 is skipped.
fn default_values(arg: &Arg) -> impl Iterator<Item = &str> {
    let delimiter = arg.get_value_delimiter();
    arg.get_default_values()
        .iter()
        .filter_map(|value| value.to_str())
        .flat_map(move |value| value.split(move |char| Some(char) == delimiter))
}

fn command_at<'cmd>(cmd: &'cmd Command, path: &[&str]) -> Option<&'cmd Command> {
    path.iter().try_fold(cmd, |level, name| level.find_subcommand(name))
}

/// Check whether the user passed `id`, on the command line or through its env var.
fn explicit(matches: &ArgMatches, id: &Id) -> bool {
    matches!(
        matches.value_source(id.as_str()),
        Some(ValueSource::CommandLine | ValueSource::EnvVariable)
    )
}

/// The args sharing an exclusive `ArgGroup` with `arg`.
fn group_mates<'cmd>(level: &'cmd Command, arg: &Arg) -> impl Iterator<Item = &'cmd Id> {
    level
        .get_groups()
        .filter(|group| {
            !ArgGroup::clone(group).is_multiple() && group.get_args().any(|id| id == arg.get_id())
        })
        .flat_map(ArgGroup::get_args)
}

/// For a flag, whether a "yes" stores `false`; `None` for an arg that takes a value.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "`ArgAction` is #[non_exhaustive]; every action but the two flags takes a value"
)]
const fn flag_negation(action: &ArgAction) -> Option<bool> {
    match action {
        ArgAction::SetTrue => Some(false),
        ArgAction::SetFalse => Some(true),
        _ => None,
    }
}

/// Set each answer as its arg's default values. No values leave the arg with no default, so
/// an emptied list drops the declared default.
fn inject<'prompted_arg>(
    cmd: Command,
    answers: impl IntoIterator<Item = (&'prompted_arg PromptedArg, &'prompted_arg [String])>,
) -> Command {
    answers.into_iter().fold(cmd, |cmd, (prompted_arg, values)| {
        with_arg_at(cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id, |arg| {
            arg.required(false).default_values(values.iter().cloned())
        })
    })
}

fn matches_at<'matches>(
    matches: &'matches ArgMatches,
    path: &[&str],
) -> Option<&'matches ArgMatches> {
    path.iter().try_fold(matches, |level, name| {
        level.subcommand().filter(|(taken, _)| taken == name).map(|(_, sub)| sub)
    })
}

/// The prompted args on the path the user took that got no value or only their default, with
/// the default's values, split at the arg's value delimiter as clap splits them. One whose
/// `default_value_if` fired is left to the arg that fired it: the conditional default beats an
/// injected answer, so asking would drop the answer.
fn pending<'prompted_arg>(
    cmd: &Command,
    matches: &ArgMatches,
    prompted_args: &'prompted_arg [PromptedArg],
) -> Vec<(&'prompted_arg PromptedArg, Vec<String>)> {
    // Built, so a flag's action has filled in its implicit default (`false` for `SetTrue`).
    let mut built = cmd.clone();
    built.build();
    prompted_args
        .iter()
        .filter_map(|prompted_arg| {
            let arg = arg_at(&built, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)?;
            let level = matches_at(matches, &prompted_arg.subcommand_path)?;
            let id = prompted_arg.attr.arg_id;
            match level.value_source(id) {
                None if default_values(arg).next().is_none() => Some((prompted_arg, Vec::new())),
                Some(ValueSource::DefaultValue) => {
                    let current = raw_values(level, id);
                    current
                        .iter()
                        .map(String::as_str)
                        .eq(default_values(arg))
                        .then_some((prompted_arg, current))
                }
                None | Some(_) => None,
            }
        })
        .collect()
}

fn raw_values(matches: &ArgMatches, id: &str) -> Vec<String> {
    matches
        .try_get_raw(id)
        .ok()
        .flatten()
        .into_iter()
        .flatten()
        .map(|value| value.to_string_lossy().into_owned())
        .collect()
}

/// Check whether `arg` takes many values, as a `Vec<T>` field does.
fn takes_many(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Append)
        || arg.get_num_args().is_some_and(|range| range.max_values() > 1)
}

/// Make every prompted arg optional, so a missing one does not stop the first parse.
fn relax(cmd: Command, prompted_args: &[PromptedArg]) -> Command {
    prompted_args.iter().fold(cmd, |cmd, prompted_arg| {
        with_arg_at(cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id, |arg| {
            arg.required(false)
        })
    })
}

/// The first line of clap's message, without its `error: ` prefix.
fn summary(err: &clap::Error) -> String {
    let rendered = err.render().to_string();
    let line = rendered.lines().next().unwrap_or_default();
    line.strip_prefix("error: ").unwrap_or(line).to_owned()
}

/// Apply `edit` to the arg `id` of the command at `path`; unchanged when either is missing.
fn with_arg_at<F>(cmd: Command, path: &[&str], id: &str, edit: F) -> Command
where
    F: FnOnce(Arg) -> Arg,
{
    match path.split_first() {
        None if cmd.get_arguments().any(|arg| arg.get_id() == id) => cmd.mut_arg(id, edit),
        Some((name, rest)) if cmd.find_subcommand(name).is_some() => {
            cmd.mut_subcommand(name, |sub| with_arg_at(sub, rest, id, edit))
        }
        None | Some(_) => cmd,
    }
}
