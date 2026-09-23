//! Parse, ask for the marked args that got no value, then parse again with the answers.
//!
//! Every answer goes back into the command as its arg's `default_value`, so clap converts and
//! validates it exactly like a typed value. Each answer is checked with a trial parse as soon as
//! it is given: the previous trial passed, so a failure can only come from the new answer, which
//! is then asked again with clap's message.

use core::iter;
use std::ffi::OsString;

use clap::{
    Arg, ArgAction, ArgGroup, ArgMatches, Command, FromArgMatches, Id, builder::PossibleValue,
    error::ErrorKind, parser::ValueSource,
};

use super::{ConfirmPrompt, Mark, PromptError, PromptNode, Prompter, SelectPrompt, TextPrompt};
use crate::parser::Failure;

/// A mark's accepted answer.
struct Answer<'mark> {
    mark: &'mark Mark,
    value: String,
}

/// One parse: the command, its argv, its marks and who to ask.
struct Session<'run> {
    argv: &'run [OsString],
    cmd: &'run Command,
    marks: &'run [Mark],
    prompter: &'run dyn Prompter,
}

impl Session<'_> {
    /// Ask for `mark` until clap accepts the answer; `None` when an optional mark is left empty.
    fn answer(
        &self,
        mark: &Mark,
        current: Option<&str>,
        answers: &[Answer<'_>],
    ) -> Result<Option<String>, PromptError> {
        let Some(arg) = arg_at(self.cmd, &mark.path, mark.spec.id) else { return Ok(None) };
        let mut error = None;
        loop {
            let Some(value) = self.ask(arg, mark, current, error.as_deref())? else {
                return Ok(None);
            };
            match self.trial(answers, mark, &value) {
                Ok(()) => return Ok(Some(value)),
                Err(err) => error = Some(summary(&err)),
            }
        }
    }

    fn ask(
        &self,
        arg: &Arg,
        mark: &Mark,
        current: Option<&str>,
        error: Option<&str>,
    ) -> Result<Option<String>, PromptError> {
        let question = mark.spec.question.map_or_else(
            || arg.get_help().map_or_else(|| mark.spec.id.to_owned(), ToString::to_string),
            str::to_owned,
        );
        let styles = self.cmd.get_styles();
        if let Some(negate) = flag_negation(arg.get_action()) {
            let set = self.prompter.confirm(&ConfirmPrompt {
                default: false,
                error,
                question: &question,
                styles,
            })?;
            return Ok(Some((set != negate).to_string()));
        }

        let options: Vec<PossibleValue> =
            arg.get_possible_values().into_iter().filter(|value| !value.is_hide_set()).collect();
        if options.is_empty() {
            return self.prompter.text(&TextPrompt {
                default: current,
                error,
                optional: !arg.is_required_set(),
                question: &question,
                styles,
            });
        }
        self.prompter
            .select(&SelectPrompt {
                default: current,
                error,
                options: &options,
                question: &question,
                styles,
            })
            .map(Some)
    }

    /// Parse with `candidate` as `mark`'s answer on top of the accepted ones.
    fn trial(
        &self,
        answers: &[Answer<'_>],
        mark: &Mark,
        candidate: &str,
    ) -> Result<(), clap::Error> {
        let accepted = answers.iter().map(|answer| (answer.mark, answer.value.as_str()));
        let cmd = inject(
            relax(self.cmd.clone(), self.marks),
            accepted.chain(iter::once((mark, candidate))),
        );
        cmd.try_get_matches_from(self.argv).map(drop)
    }
}

/// Parse `argv` against `cmd`, asking `prompter` for the marks in `tree` that got no value.
pub(crate) fn run<T>(
    cmd: Command,
    tree: &'static PromptNode,
    argv: &[OsString],
    prompter: &dyn Prompter,
) -> Result<T, Failure>
where
    T: FromArgMatches,
{
    let marks = tree.marks();
    if marks.is_empty() || !prompter.is_available() {
        return finish(cmd, argv, &[]);
    }
    // Help, usage and errors come from the command the user sees, not the relaxed one.
    let Ok(matches) = relax(cmd.clone(), &marks).try_get_matches_from(argv) else {
        return finish(cmd, argv, &[]);
    };

    let session = Session { argv, cmd: &cmd, marks: &marks, prompter };
    let mut answers = Vec::new();
    for (mark, current) in pending(&cmd, &matches, &marks) {
        if blocked(&cmd, &matches, mark, &answers) {
            continue;
        }
        match session.answer(mark, current.as_deref(), &answers) {
            Ok(Some(value)) => answers.push(Answer { mark, value }),
            Ok(None) => {}
            Err(PromptError::Cancelled) => return Err(Failure::Cancelled),
            Err(PromptError::Io(err)) => {
                return Err(Failure::Clap(clap::Error::raw(ErrorKind::Io, err)));
            }
            Err(PromptError::NotATerminal) => return finish(cmd, argv, &[]),
        }
    }
    finish(cmd, argv, &answers)
}

fn arg_at<'cmd>(cmd: &'cmd Command, path: &[&str], id: &str) -> Option<&'cmd Arg> {
    command_at(cmd, path)?.get_arguments().find(|arg| arg.get_id() == id)
}

/// Check whether `mark` conflicts with an arg the user passed or an earlier answer.
///
/// Answers go in as default values, which clap never checks for conflicts, so asking here
/// would let the parse land in a state clap rejects when typed.
fn blocked(cmd: &Command, matches: &ArgMatches, mark: &Mark, answers: &[Answer<'_>]) -> bool {
    let (Some(level), Some(level_matches)) =
        (command_at(cmd, &mark.path), matches_at(matches, &mark.path))
    else {
        return false;
    };
    let Some(arg) = level.get_arguments().find(|arg| arg.get_id() == mark.spec.id) else {
        return false;
    };
    let answered = |id: &Id| {
        answers.iter().any(|answer| answer.mark.path == mark.path && answer.mark.spec.id == id)
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

/// Parse `argv` against `cmd` with `answers` injected, then build `T`.
fn finish<T>(cmd: Command, argv: &[OsString], answers: &[Answer<'_>]) -> Result<T, Failure>
where
    T: FromArgMatches,
{
    let mut cmd = inject(cmd, answers.iter().map(|answer| (answer.mark, answer.value.as_str())));
    let mut matches = cmd.try_get_matches_from_mut(argv).map_err(Failure::Clap)?;
    T::from_arg_matches_mut(&mut matches).map_err(|err| Failure::Clap(err.format(&mut cmd)))
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

/// Set each answer as its arg's default value.
fn inject<'mark>(
    cmd: Command,
    answers: impl IntoIterator<Item = (&'mark Mark, &'mark str)>,
) -> Command {
    answers.into_iter().fold(cmd, |cmd, (mark, value)| {
        let value = value.to_owned();
        with_arg_at(cmd, &mark.path, mark.spec.id, |arg| arg.required(false).default_value(value))
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

/// The marks on the path the user took that got no value or only their default, with the
/// default's value.
fn pending<'mark>(
    cmd: &Command,
    matches: &ArgMatches,
    marks: &'mark [Mark],
) -> Vec<(&'mark Mark, Option<String>)> {
    marks
        .iter()
        .filter(|mark| arg_at(cmd, &mark.path, mark.spec.id).is_some())
        .filter_map(|mark| {
            let level = matches_at(matches, &mark.path)?;
            let id = mark.spec.id;
            match level.value_source(id) {
                None => Some((mark, None)),
                Some(ValueSource::DefaultValue) => Some((mark, raw_value(level, id))),
                Some(_) => None,
            }
        })
        .collect()
}

fn raw_value(matches: &ArgMatches, id: &str) -> Option<String> {
    let mut raw = matches.try_get_raw(id).ok().flatten()?;
    raw.next().map(|value| value.to_string_lossy().into_owned())
}

/// Make every mark optional, so a missing one does not stop the first parse.
fn relax(cmd: Command, marks: &[Mark]) -> Command {
    marks.iter().fold(cmd, |cmd, mark| {
        with_arg_at(cmd, &mark.path, mark.spec.id, |arg| arg.required(false))
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
