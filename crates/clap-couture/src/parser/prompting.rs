//! Parse, ask for the prompted args that got no value, then parse again with the answers.
//!
//! Every answer goes back into the command as its arg's `default_value`, so clap converts and
//! validates it exactly like a typed value. Each answer is checked with a trial parse as soon as
//! it is given: the previous trial passed, so a failure can only come from the new answer, which
//! is then asked again with clap's message.

use core::iter;
use std::ffi::OsString;

use clap::{
    Arg, ArgAction, ArgGroup, ArgMatches, Command, FromArgMatches, Id,
    builder::{PossibleValue, Resettable},
    error::ErrorKind,
    parser::ValueSource,
};

use super::Failure;
#[cfg(feature = "interactive-editor")]
use crate::interactive::{EditorAttr, EditorPrompt};
use crate::{
    Messages,
    interactive::{
        Backend, CommandStyles, ConfirmPrompt, PromptError, PromptedArg, PromptedArgTree,
        SelectPrompt, TextPrompt,
    },
};

/// A prompted arg's accepted answer.
struct Answer<'prompted_arg> {
    prompted_arg: &'prompted_arg PromptedArg,
    value: String,
}

/// A prompted arg about to be asked, and what is known about it.
struct Asked<'ask> {
    arg: &'ask Arg,
    /// Its default's value, pre-filled.
    current: Option<&'ask str>,
    /// Leaving it unset passes clap, so an empty answer is allowed.
    optional: bool,
    prompted_arg: &'ask PromptedArg,
}

/// An answer clap turned down, and why; an editor prompt reopens with it.
#[cfg_attr(
    not(feature = "interactive-editor"),
    expect(dead_code, reason = "only an editor prompt reopens with the rejected answer")
)]
struct Rejected {
    error: String,
    value: String,
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
    /// Ask for `prompted_arg` until clap accepts the answer; `None` when an optional one is left
    /// empty. `later` are the prompted args still to be asked after it.
    fn answer(
        &self,
        prompted_arg: &PromptedArg,
        current: Option<&str>,
        answers: &[Answer<'_>],
        later: &[&PromptedArg],
    ) -> Result<Option<String>, PromptError> {
        let Some(arg) = arg_at(self.cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
        else {
            return Ok(None);
        };
        let asked = Asked { arg, current, optional: !self.required(answers, later), prompted_arg };
        let mut rejected = None;
        loop {
            let Some(value) = self.ask(&asked, rejected.as_ref())? else {
                return Ok(None);
            };
            match self.trial(answers, prompted_arg, &value) {
                Ok(()) => return Ok(Some(value)),
                Err(err) => {
                    let error = summary(&err, &value);
                    self.backend.report(&error, CommandStyles(self.cmd.get_styles()))?;
                    rejected = Some(Rejected { error, value });
                }
            }
        }
    }

    #[cfg_attr(
        not(feature = "interactive-editor"),
        expect(
            unused_variables,
            reason = "only an editor prompt reopens with the rejected answer"
        )
    )]
    fn ask(
        &self,
        asked: &Asked<'_>,
        rejected: Option<&Rejected>,
    ) -> Result<Option<String>, PromptError> {
        let Asked { arg, current, optional, prompted_arg } = *asked;
        let question = prompted_arg.attr.question.as_ref().map_or_else(
            || {
                arg.get_help()
                    .map_or_else(|| prompted_arg.attr.arg_id.to_owned(), ToString::to_string)
            },
            |question| question.get().into_owned(),
        );
        let styles = CommandStyles(self.cmd.get_styles());
        // A flag has nothing to edit, so one that slipped past the derive asks as a flag.
        #[cfg(feature = "interactive-editor")]
        if let Some(editor) =
            prompted_arg.attr.editor.as_ref().filter(|_| arg.get_action().takes_values())
        {
            return self.ask_in_editor(asked, editor, &question, rejected);
        }
        if let Some(negate) = flag_negation(arg.get_action()) {
            let set = self.backend.confirm(&ConfirmPrompt {
                default: false,
                question: &question,
                styles,
            })?;
            return Ok(Some((set != negate).to_string()));
        }

        let options: Vec<PossibleValue> =
            arg.get_possible_values().into_iter().filter(|value| !value.is_hide_set()).collect();
        if options.is_empty() {
            return self.backend.text(&TextPrompt {
                default: current,
                optional,
                question: &question,
                styles,
            });
        }
        self.backend.select(&SelectPrompt {
            default: current,
            none_option: &self.messages.none.get(),
            optional,
            options: &options,
            question: &question,
            styles,
        })
    }

    /// Ask in the user's editor, starting from the rejected answer if there is one. An empty
    /// answer to a required arg aborts, as git aborts on an empty commit message, and so does one
    /// to an arg with a default, which an empty answer would quietly bring back.
    #[cfg(feature = "interactive-editor")]
    fn ask_in_editor(
        &self,
        asked: &Asked<'_>,
        editor: &EditorAttr,
        question: &str,
        rejected: Option<&Rejected>,
    ) -> Result<Option<String>, PromptError> {
        let Asked { arg, current, optional: _, prompted_arg } = *asked;
        let styles = CommandStyles(self.cmd.get_styles());
        let optional = asked.optional && arg.get_default_values().is_empty();
        let instructions = if optional {
            self.messages.editor_instructions_optional.get()
        } else {
            self.messages.editor_instructions.get()
        };
        let answer = self.backend.editor(&EditorPrompt {
            default: rejected.map_or(current, |rejected| Some(rejected.value.as_str())),
            error: rejected.map(|rejected| rejected.error.as_str()),
            extension: editor.extension,
            file_stem: &iter::once(self.cmd.get_name())
                .chain(prompted_arg.subcommand_path.iter().copied())
                .chain(iter::once(prompted_arg.attr.arg_id))
                .collect::<Vec<_>>()
                .join("-"),
            instructions: &instructions,
            not_utf8: &self.messages.editor_not_utf8.get(),
            optional,
            question,
            returned_early: &self.messages.editor_returned_early.get(),
            styles,
            wait_hint: &self.messages.editor_wait_hint.get(),
            waiting: &self.messages.editor_waiting.get(),
        })?;
        if answer.is_none() && !optional {
            self.backend.report(&self.messages.editor_empty.get(), styles)?;
            return Err(PromptError::Cancelled);
        }
        Ok(answer)
    }

    /// Check whether leaving the arg being asked unset fails the parse: it is `required`, or
    /// required through `required_unless_present` or a required group. The accepted answers go
    /// in, and only the args still to be asked, `later`, stay relaxed, so a group another of them
    /// can still satisfy doesn't make this one required.
    fn required(&self, answers: &[Answer<'_>], later: &[&PromptedArg]) -> bool {
        let accepted = answers.iter().map(|answer| (answer.prompted_arg, answer.value.as_str()));
        let cmd = inject(relax(self.cmd.clone(), later.iter().copied()), accepted);
        cmd.try_get_matches_from(self.argv)
            .is_err_and(|err| err.kind() == ErrorKind::MissingRequiredArgument)
    }

    /// Parse with `candidate` as `prompted_arg`'s answer on top of the accepted ones.
    fn trial(
        &self,
        answers: &[Answer<'_>],
        prompted_arg: &PromptedArg,
        candidate: &str,
    ) -> Result<(), clap::Error> {
        let accepted = answers.iter().map(|answer| (answer.prompted_arg, answer.value.as_str()));
        let cmd = inject(
            relax(self.cmd.clone(), self.prompted_args.iter()),
            accepted.chain(iter::once((prompted_arg, candidate))),
        );
        cmd.try_get_matches_from(self.argv).map(drop)
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
    let Ok(matches) = relax(cmd.clone(), prompted_args.iter()).try_get_matches_from(argv) else {
        return super::parse(cmd, argv);
    };

    let session = Session { argv, backend, cmd: &cmd, messages, prompted_args: &prompted_args };
    let mut answers = Vec::new();
    let pending = pending(&cmd, &matches, &prompted_args);
    for (index, (prompted_arg, current)) in pending.iter().enumerate() {
        if blocked(&cmd, &matches, prompted_arg, &answers) {
            continue;
        }
        let later: Vec<&PromptedArg> =
            pending.iter().skip(index.saturating_add(1)).map(|(later, _)| *later).collect();
        match session.answer(prompted_arg, current.as_deref(), &answers, &later) {
            Ok(Some(value)) => answers.push(Answer { prompted_arg, value }),
            Ok(None) => {}
            Err(PromptError::Cancelled) => return Err(Failure::Cancelled { signal: None }),
            Err(PromptError::Signalled(signal)) => {
                return Err(Failure::Cancelled { signal: Some(signal) });
            }
            Err(PromptError::Io(err)) => return Err(Failure::Io(err)),
            Err(PromptError::NotATerminal) => return super::parse(cmd, argv),
        }
    }
    let cmd_with_answers =
        inject(cmd, answers.iter().map(|answer| (answer.prompted_arg, answer.value.as_str())));
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

/// The command at `path` passed through `edit`; unchanged when the path is missing.
fn with_command_at<F>(cmd: Command, path: &[&str], edit: F) -> Command
where
    F: FnOnce(Command) -> Command,
{
    match path.split_first() {
        None => edit(cmd),
        Some((name, rest)) if cmd.find_subcommand(name).is_some() => {
            cmd.mut_subcommand(name, |sub| with_command_at(sub, rest, edit))
        }
        Some(_) => cmd,
    }
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

/// Set each answer as its arg's default value.
fn inject<'prompted_arg>(
    cmd: Command,
    answers: impl IntoIterator<Item = (&'prompted_arg PromptedArg, &'prompted_arg str)>,
) -> Command {
    answers.into_iter().fold(cmd, |cmd, (prompted_arg, value)| {
        let value = value.to_owned();
        // A default value is no explicit value to clap, so the rules that require the arg would
        // still miss it.
        with_arg_at(
            unrequire(cmd, prompted_arg),
            &prompted_arg.subcommand_path,
            prompted_arg.attr.arg_id,
            |arg| arg.default_value(value),
        )
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
/// the default's value. One whose `default_value_if` fired is left to the arg that fired it: the
/// conditional default beats an injected answer, so asking would drop the answer.
fn pending<'prompted_arg>(
    cmd: &Command,
    matches: &ArgMatches,
    prompted_args: &'prompted_arg [PromptedArg],
) -> Vec<(&'prompted_arg PromptedArg, Option<String>)> {
    // Built, so a flag's action has filled in its implicit default (`false` for `SetTrue`).
    let mut built = cmd.clone();
    built.build();
    prompted_args
        .iter()
        .filter_map(|prompted_arg| {
            let arg = arg_at(&built, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)?;
            let level = matches_at(matches, &prompted_arg.subcommand_path)?;
            let id = prompted_arg.attr.arg_id;
            let default =
                arg.get_default_values().first().map(|value| value.to_string_lossy().into_owned());
            match level.value_source(id) {
                None if default.is_none() => Some((prompted_arg, None)),
                Some(ValueSource::DefaultValue) => {
                    let current = raw_value(level, id);
                    (current == default).then_some((prompted_arg, current))
                }
                None | Some(_) => None,
            }
        })
        .collect()
}

fn raw_value(matches: &ArgMatches, id: &str) -> Option<String> {
    let mut raw = matches.try_get_raw(id).ok().flatten()?;
    raw.next().map(|value| value.to_string_lossy().into_owned())
}

/// Make each of `prompted_args` optional, so a missing one does not stop a parse.
fn relax<'prompted_arg>(
    cmd: Command,
    prompted_args: impl IntoIterator<Item = &'prompted_arg PromptedArg>,
) -> Command {
    prompted_args.into_iter().fold(cmd, unrequire)
}

/// The first line of clap's message, without its `error: ` prefix. clap quotes the rejected
/// `value`, so a multi-line one is shortened to its first line first, keeping the reason after it.
fn summary(err: &clap::Error, value: &str) -> String {
    let mut rendered = err.render().to_string();
    if let Some((first_line, _)) = value.split_once('\n') {
        rendered = rendered.replace(value, &format!("{first_line}\u{2026}"));
    }
    let line = rendered.lines().next().unwrap_or_default();
    line.strip_prefix("error: ").unwrap_or(line).to_owned()
}

/// Drop the rules that make `prompted_arg` required: its `required`, its
/// `required_unless_present`, and the `required` of the groups it belongs to. clap can't clear
/// `required_unless_present_all` or `required_if_eq`, so those stay.
fn unrequire(cmd: Command, prompted_arg: &PromptedArg) -> Command {
    let id = prompted_arg.attr.arg_id;
    with_command_at(cmd, &prompted_arg.subcommand_path, |level| {
        if !level.get_arguments().any(|arg| arg.get_id() == id) {
            return level;
        }
        let groups: Vec<Id> = level
            .get_groups()
            .filter(|group| group.is_required_set() && group.get_args().any(|arg| arg == id))
            .map(|group| group.get_id().clone())
            .collect();
        let level =
            level.mut_arg(id, |arg| arg.required(false).required_unless_present(Resettable::Reset));
        groups
            .iter()
            .fold(level, |level, group| level.mut_group(group, |group| group.required(false)))
    })
}

/// Apply `edit` to the arg `id` of the command at `path`; unchanged when either is missing.
fn with_arg_at<F>(cmd: Command, path: &[&str], id: &str, edit: F) -> Command
where
    F: FnOnce(Arg) -> Arg,
{
    with_command_at(cmd, path, |level| {
        if level.get_arguments().any(|arg| arg.get_id() == id) {
            level.mut_arg(id, edit)
        } else {
            level
        }
    })
}
