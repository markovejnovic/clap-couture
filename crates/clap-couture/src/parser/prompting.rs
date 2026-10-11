//! Parse, ask for the prompted args that got no value, then parse again with the answers.
//!
//! Every answer goes back into the command as its arg's `default_values`, so clap converts and
//! validates it exactly like a typed value. Each answer is checked with a trial parse as soon as
//! it is given: the previous trial passed, so a failure can only come from the new answer, which
//! is then asked again with clap's message. For an arg asked one value at a time, each value is
//! an answer of its own, checked on top of the values before it.
//!
//! Confirmations come last, so their questions can name the answers. A declined one fails the
//! parse; without a terminal to ask on, one left unanswered fails it as a missing required arg.
//!
//! A secret's answer goes into a vault instead, and clap gets a placeholder that the secret's
//! value parser swaps back. It skips the trial parse: that parser takes any string, so there is
//! nothing to ask again.
//!
//! An arg passed `-` that `dash` makes ask is asked like one left out, but its answer fills the
//! `-` through its value parser instead, as the command line beats a default.
//!
//! Before any of it, the `#[couture(stdin)]` arg on the path the user took takes stdin: all of it
//! when passed `-`, else what is piped, if it got no value. It is then no longer asked.

use core::iter;
use std::ffi::{OsStr, OsString};

use clap::{
    Arg, ArgAction, ArgGroup, ArgMatches, Command, FromArgMatches, Id,
    builder::{PossibleValue, Resettable},
    error::ErrorKind,
    parser::ValueSource,
};

use super::{
    ParseError,
    dash::{DashArgs, Dashed},
    secrets::{self, Vault},
    stdin::{StdinArgs, StdinValue},
};
use crate::{
    Messages,
    interactive::{
        Backend, CommandStyles, ConfirmPrompt, MultiSelectPrompt, PromptError, PromptPurpose,
        PromptedArg, PromptedArgTree, SecretPrompt, SelectPrompt, TextPrompt,
    },
    os::{signal::Signal, stdin::Stdin},
};
#[cfg(feature = "interactive-editor")]
use crate::{
    Text,
    interactive::{EditorAttr, EditorPrompt},
};

/// A prompted arg's accepted answer.
pub(super) struct Answer<'prompted_arg> {
    /// What clap gets as the arg's defaults: the answer itself, or a secret's placeholder. One
    /// value, or any number for an arg that takes many.
    injected_values: Vec<String>,
    prompted_arg: &'prompted_arg PromptedArg,
}

/// A prompted arg about to be asked in the editor, and what is known about it.
#[cfg(feature = "interactive-editor")]
struct Asked<'ask> {
    arg: &'ask Arg,
    /// Its default's value, pre-filled.
    current: Option<&'ask str>,
    /// Leaving it unset passes clap, so an empty answer is allowed.
    optional: bool,
    prompted_arg: &'ask PromptedArg,
    question: &'ask str,
}

/// An answer clap turned down, and why; an editor prompt reopens with it.
#[cfg(feature = "interactive-editor")]
struct Rejected {
    error: String,
    value: String,
}

/// [`Command`], named after the binary that runs it.
trait BinNamed {
    /// `self` with the bin name clap takes from `argv[0]`, so couture's own errors show it too.
    fn bin_named(self, argv: &[OsString]) -> Self;
}

impl BinNamed for Command {
    fn bin_named(self, argv: &[OsString]) -> Self {
        // As clap's own parse names it: a multicall command or one set without a binary name is
        // named by clap alone, and a name set already is kept.
        if self.is_multicall_set() || self.is_no_binary_name_set() || self.get_bin_name().is_some()
        {
            return self;
        }
        let program_name = argv.first().and_then(|called| crate::os::program_name(called));
        match program_name.and_then(OsStr::to_str) {
            Some(bin_name) => self.bin_name(bin_name.to_owned()),
            None => self,
        }
    }
}

/// One parse: the command, its argv, its prompted args, who to ask and where secrets go.
struct Session<'run> {
    argv: &'run [OsString],
    backend: &'run dyn Backend,
    cmd: &'run Command,
    /// The args that take `-`; an answer to one passed `-` fills its slot.
    dash_args: &'run DashArgs,
    /// The args passed `-` to be asked for.
    dashed: &'run Dashed<'run>,
    messages: &'run Messages,
    prompted_args: &'run [PromptedArg],
    /// Holds the secret answers; `cmd` resolves their placeholders from it.
    vault: &'run Vault,
}

impl Session<'_> {
    /// Ask for `prompted_arg` until clap accepts the answer; `None` when an optional single-value
    /// one is left empty. `current` is the arg's default, split into its values. `later` are the
    /// prompted args still to be asked after it. A secret is asked once and comes back as its vault
    /// placeholder.
    fn answer(
        &self,
        prompted_arg: &PromptedArg,
        current: &[String],
        answers: &[Answer<'_>],
        later: &[&PromptedArg],
    ) -> Result<Option<Vec<String>>, PromptError> {
        let Some(arg) = arg_at(self.cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
        else {
            return Ok(None);
        };
        // An arg passed `-` was asked for by the user, so it takes an answer.
        let dashed = self.dashed.asks(&prompted_arg.subcommand_path, prompted_arg.attr.arg_id);
        let optional = !dashed && !self.required(answers, later);
        let question = self.question(prompted_arg);
        if prompted_arg.attr.secret.is_some() {
            // clap has nothing to reject in a string, so a secret skips the trial parse; the vault
            // keeps it and clap gets a placeholder. `current` goes unused: the derive refuses a
            // default on a secret, and showing one would echo it.
            let secret = self.backend.secret(&SecretPrompt {
                optional,
                question: &question,
                styles: CommandStyles(self.cmd.get_styles()),
            })?;
            let placeholder = secret.map(|secret| vec![self.vault.store(secret)]);
            // Every other answer fills its `-` in its trial parse, which a secret skips.
            if let Some(placeholder) = &placeholder {
                self.dash_args.fill(prompted_arg, placeholder);
            }
            return Ok(placeholder);
        }
        // A flag has nothing to edit, so one that slipped past the derive asks as a flag.
        #[cfg(feature = "interactive-editor")]
        if let Some(editor) = prompted_arg
            .attr
            .editor
            .as_ref()
            .filter(|_| arg.get_action().takes_values() && !takes_many(arg))
        {
            let asked = Asked {
                arg,
                current: current.first().map(String::as_str),
                optional,
                prompted_arg,
                question: &question,
            };
            return self.in_editor(&asked, editor, answers);
        }
        let options: Vec<PossibleValue> =
            arg.get_possible_values().into_iter().filter(|value| !value.is_hide_set()).collect();
        if !takes_many(arg) {
            return self.until_accepted(answers, prompted_arg, || {
                let value = self.ask(arg, &question, &options, current.first(), optional)?;
                Ok(value.map(|value| vec![value]))
            });
        }
        if options.is_empty() {
            return self.each_value(&question, prompted_arg, current, answers, optional).map(Some);
        }
        self.until_accepted(answers, prompted_arg, || {
            self.backend
                .multi_select(&MultiSelectPrompt {
                    defaults: current,
                    optional,
                    options: &options,
                    question: &question,
                    selection_required: &self.messages.selection_required.get(),
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
        optional: bool,
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
                optional,
                question,
                styles,
            });
        }
        self.backend.select(&SelectPrompt {
            default: current.map(String::as_str),
            none_option: &self.messages.none.get(),
            optional,
            options,
            question,
            styles,
        })
    }

    /// Ask in the user's editor, starting from the rejected answer, else the arg's default, else
    /// the editor's `initial` text. An empty
    /// answer to a required arg aborts, as git aborts on an empty commit message, and so does one
    /// to an arg with a default, which an empty answer would quietly bring back.
    #[cfg(feature = "interactive-editor")]
    fn ask_in_editor(
        &self,
        asked: &Asked<'_>,
        editor: &EditorAttr,
        rejected: Option<&Rejected>,
    ) -> Result<Option<String>, PromptError> {
        let Asked { arg, current, optional: _, prompted_arg, question } = *asked;
        let styles = CommandStyles(self.cmd.get_styles());
        let optional = asked.optional && arg.get_default_values().is_empty();
        let instructions = if optional {
            self.messages.editor_instructions_optional.get()
        } else {
            self.messages.editor_instructions.get()
        };
        let initial = editor.initial.as_ref().map(Text::get);
        let answer = self.backend.editor(&EditorPrompt {
            initial: rejected
                .map(|rejected| rejected.value.as_str())
                .or(current)
                .or(initial.as_deref()),
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
            waiting: &self.messages.editor_waiting.get(),
        })?;
        if answer.is_none() && !optional {
            self.backend.report(&self.messages.editor_empty.get(), styles)?;
            return Err(PromptError::Cancelled);
        }
        Ok(answer)
    }

    /// Ask whether to go ahead, with each `{id}` in the question filled in from the args as
    /// passed and answered so far.
    fn confirm(
        &self,
        prompted_arg: &PromptedArg,
        answers: &[Answer<'_>],
    ) -> Result<bool, PromptError> {
        let question = self.question(prompted_arg);
        let answered = inject(
            relax(self.cmd.clone(), self.prompted_args),
            answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice())),
        )
        .try_get_matches_from(self.argv);
        let question = match answered
            .as_ref()
            .ok()
            .and_then(|answered| matches_at(answered, &prompted_arg.subcommand_path))
        {
            Some(level) => interpolate(&question, |id| {
                self.dashed
                    .text(&prompted_arg.subcommand_path, id)
                    .map(|text| vec![text])
                    .or_else(|| level.try_get_raw(id).ok().map(|_| raw_values(level, id)))
            }),
            None => question,
        };
        self.backend.confirm(&ConfirmPrompt {
            default: false,
            question: &question,
            styles: CommandStyles(self.cmd.get_styles()),
        })
    }

    /// Ask for one value at a time until an empty answer, each starting out as the default's
    /// value at its position. Only a required arg's first value may not be empty.
    ///
    /// The default goes in as initial text rather than as the answer Enter accepts, so clearing it
    /// still finishes the list.
    fn each_value(
        &self,
        question: &str,
        prompted_arg: &PromptedArg,
        current: &[String],
        answers: &[Answer<'_>],
        optional: bool,
    ) -> Result<Vec<String>, PromptError> {
        let finish_hint = self.messages.finish_hint.get();
        let mut values = Vec::new();
        loop {
            let optional = !values.is_empty() || optional;
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
                self.backend
                    .report(&summary(&err, &values), CommandStyles(self.cmd.get_styles()))?;
                values.pop();
            }
        }
    }

    /// Ask in the editor until clap accepts the answer, reopening with the rejected one.
    #[cfg(feature = "interactive-editor")]
    fn in_editor(
        &self,
        asked: &Asked<'_>,
        editor: &EditorAttr,
        answers: &[Answer<'_>],
    ) -> Result<Option<Vec<String>>, PromptError> {
        let mut rejected = None;
        loop {
            let Some(value) = self.ask_in_editor(asked, editor, rejected.as_ref())? else {
                return Ok(None);
            };
            let values = vec![value];
            match self.trial(answers, asked.prompted_arg, &values) {
                Ok(()) => return Ok(Some(values)),
                Err(err) => {
                    let error = summary(&err, &values);
                    self.backend.report(&error, CommandStyles(self.cmd.get_styles()))?;
                    rejected = values.into_iter().next().map(|value| Rejected { error, value });
                }
            }
        }
    }

    /// The `= ...` override, else the arg's help text, else its id.
    fn question(&self, prompted_arg: &PromptedArg) -> String {
        prompted_arg.attr.question.as_ref().map_or_else(
            || {
                arg_at(self.cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
                    .and_then(Arg::get_help)
                    .map_or_else(|| prompted_arg.attr.arg_id.to_owned(), ToString::to_string)
            },
            |question| question.get().into_owned(),
        )
    }

    /// Check whether leaving the arg being asked unset fails the parse: it is `required`, or
    /// required through `required_unless_present` or a required group. The accepted answers go
    /// in, and only the args still to be asked, `later`, stay relaxed, so a group another of them
    /// can still satisfy doesn't make this one required.
    fn required(&self, answers: &[Answer<'_>], later: &[&PromptedArg]) -> bool {
        let accepted =
            answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice()));
        let cmd = inject(relax(self.cmd.clone(), later.iter().copied()), accepted);
        cmd.try_get_matches_from(self.argv)
            .is_err_and(|err| err.kind() == ErrorKind::MissingRequiredArgument)
    }

    /// Parse with `candidate` as `prompted_arg`'s answer on top of the accepted ones.
    fn trial(
        &self,
        answers: &[Answer<'_>],
        prompted_arg: &PromptedArg,
        candidate: &[String],
    ) -> Result<(), clap::Error> {
        let accepted =
            answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice()));
        let with_candidate: Vec<_> =
            accepted.chain(iter::once((prompted_arg, candidate))).collect();
        for (answered, values) in &with_candidate {
            self.dash_args.fill(answered, values);
        }
        let cmd = inject(relax(self.cmd.clone(), self.prompted_args.iter()), with_candidate);
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
                    let error = summary(&err, &values);
                    self.backend.report(&error, CommandStyles(self.cmd.get_styles()))?;
                }
            }
        }
    }
}

/// Parse `argv` against `cmd`, filling its stdin arg from `stdin`, asking `backend` for the
/// prompted args that got no value, and standing in for each `-` a dash arg got what was asked or
/// read for it.
pub(super) fn parse<T>(
    cmd: Command,
    prompted_arg_tree: &'static PromptedArgTree,
    messages: &Messages,
    argv: &[OsString],
    backend: &dyn Backend,
    stdin: &impl Stdin,
) -> Result<T, ParseError>
where
    T: FromArgMatches,
{
    let cmd = cmd.bin_named(argv);
    let mut prompted_args = prompted_arg_tree.prompted_args();
    let stdin_args = StdinArgs::of(prompted_arg_tree);
    let dash_args = DashArgs::of(&prompted_args, &stdin_args);
    let nothing_to_ask = prompted_args.is_empty() || !backend.is_available();
    if nothing_to_ask && stdin_args.is_empty() && dash_args.is_empty() {
        return parse_unprompted(cmd, &prompted_args, argv);
    }
    // Re-guarded with the vault this parse's answers go into; trial and final parses share it.
    let vault = Vault::default();
    let mut cmd = secrets::guard(cmd, &prompted_args, &vault, &messages.environment.get());
    let relaxed = stdin_args.unrequired(relax(dash_args.checking(cmd.clone()), &prompted_args));
    // Prompts read the terminal, so a `-` that may ask or read stdin reads it when it is no
    // terminal, as when piped.
    let can_ask = stdin.is_terminal() && backend.is_available();
    // Help, usage and errors come from clap, with `-` let through.
    let Ok(matches) = relaxed.try_get_matches_from(argv) else {
        return super::parse(dash_args.checking(cmd), argv);
    };
    let dashed = dash_args.passed(&matches, can_ask);
    // `-` is the only way to ask a `prompt(dash)` arg.
    prompted_args.retain(|prompted_arg| {
        prompted_arg.attr.dash.is_none()
            || dashed.asks(&prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
    });
    if let Some(asked) = dashed.first_asked().filter(|_| !backend.is_available()) {
        return Err(asked.unaskable(&cmd));
    }
    let checking = dash_args.checking(cmd.clone());
    if let Some(StdinValue { stdin_arg, text }) =
        stdin_args.read(&checking, &prompted_args, &dashed, argv, stdin)?
    {
        let (path, id) = (stdin_arg.subcommand_path.as_slice(), stdin_arg.attr.arg_id);
        let is_stdin_arg = |prompted_arg: &PromptedArg| {
            prompted_arg.subcommand_path == path && prompted_arg.attr.arg_id == id
        };
        let secret = prompted_args
            .iter()
            .any(|prompted| is_stdin_arg(prompted) && prompted.attr.secret.is_some());
        let text = if secret { vault.store(text.into()) } else { text };
        if !dashed.fill_read(path, id, text.clone()) {
            cmd = inject_at(cmd, path, id, &[text]);
        }
        prompted_args.retain(|prompted_arg| !is_stdin_arg(prompted_arg));
    }
    if prompted_args.is_empty() || !backend.is_available() {
        return parse_unprompted(dash_args.typed(cmd), &prompted_args, argv);
    }
    let checking = dash_args.checking(cmd.clone());
    // Help, usage and errors come from the command the user sees, not the relaxed one.
    let Ok(matches) = relax(checking.clone(), prompted_args.iter()).try_get_matches_from(argv)
    else {
        return super::parse(checking, argv);
    };

    let session = Session {
        argv,
        backend,
        cmd: &checking,
        dash_args: &dash_args,
        dashed: &dashed,
        messages,
        prompted_args: &prompted_args,
        vault: &vault,
    };
    let mut answers = Vec::new();
    let mut unanswered = pending(&checking, &matches, &prompted_args, &dashed);
    // Stable, so values and confirmations each keep declaration order.
    unanswered
        .sort_by_key(|(prompted_arg, _)| prompted_arg.attr.purpose == PromptPurpose::Confirmation);
    for (index, (prompted_arg, current)) in unanswered.iter().enumerate() {
        let (path, id) = (prompted_arg.subcommand_path.as_slice(), prompted_arg.attr.arg_id);
        if blocked(&checking, &matches, path, id, &answers) {
            continue;
        }
        let later: Vec<&PromptedArg> =
            unanswered.iter().skip(index.saturating_add(1)).map(|(later, _)| *later).collect();
        let answer = match prompted_arg.attr.purpose {
            PromptPurpose::Confirmation => match session.confirm(prompted_arg, &answers) {
                Ok(false) => return Err(ParseError::Declined),
                confirmed => confirmed.map(|_| Some(Vec::from(["true".to_owned()]))),
            },
            PromptPurpose::Value => session.answer(prompted_arg, current, &answers, &later),
        };
        match answer {
            Ok(Some(injected_values)) => answers.push(Answer { injected_values, prompted_arg }),
            Ok(None) => {}
            // Backing out counts as Ctrl-C.
            Err(PromptError::Cancelled) => return Err(ParseError::Cancelled(Signal::Interrupt)),
            Err(PromptError::Signalled(signal)) => return Err(ParseError::Cancelled(signal)),
            Err(PromptError::Io(err)) => return Err(ParseError::Io(err)),
            Err(PromptError::NotATerminal) => {
                if let Some(asked) = dashed.first_asked() {
                    return Err(asked.unaskable(&cmd));
                }
                return parse_unprompted(dash_args.typed(cmd), &prompted_args, argv);
            }
        }
    }
    let cmd_with_answers = inject(
        cmd,
        answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice())),
    );
    super::parse(dash_args.typed(cmd_with_answers), argv)
}

/// Parse as clap would, without asking. A confirmation left unanswered fails the parse with
/// clap's error for a missing required arg, so only passing its flag gets past it.
fn parse_unprompted<T>(
    mut cmd: Command,
    prompted_args: &[PromptedArg],
    argv: &[OsString],
) -> Result<T, ParseError>
where
    T: FromArgMatches,
{
    let confirmations: Vec<_> = prompted_args
        .iter()
        .filter(|prompted_arg| prompted_arg.attr.purpose == PromptPurpose::Confirmation)
        .collect();
    if confirmations.is_empty() {
        return super::parse(cmd, argv);
    }
    // Kept unbuilt: requiring an arg on the built command makes clap show help instead.
    let declared = cmd.clone();
    let mut matches = cmd.try_get_matches_from_mut(argv).map_err(ParseError::Clap)?;
    let unconfirmed = confirmations.iter().any(|confirmation| {
        let id = confirmation.attr.arg_id;
        let passed = matches_at(&matches, &confirmation.subcommand_path)
            .is_none_or(|level| explicit(level, &Id::from(id)));
        !passed && !blocked(&cmd, &matches, &confirmation.subcommand_path, id, &[])
    });
    if unconfirmed {
        // Parsed again only to fail: clap words the error, and the help stays as declared.
        let required = confirmations.iter().fold(declared, |cmd, confirmation| {
            with_arg_at(cmd, &confirmation.subcommand_path, confirmation.attr.arg_id, |arg| {
                arg.required(true)
            })
        });
        return super::parse(required, argv);
    }
    T::from_arg_matches_mut(&mut matches).map_err(|err| ParseError::Clap(err.format(&mut cmd)))
}

pub(super) fn arg_at<'cmd>(cmd: &'cmd Command, path: &[&str], id: &str) -> Option<&'cmd Arg> {
    command_at(cmd, path)?.get_arguments().find(|arg| arg.get_id() == id)
}

/// Check whether arg `id` of the command at `path` conflicts with an arg the user passed or an
/// earlier answer.
///
/// Answers go in as default values, which clap never checks for conflicts, so asking here
/// would let the parse land in a state clap rejects when typed.
pub(super) fn blocked(
    cmd: &Command,
    matches: &ArgMatches,
    path: &[&str],
    id: &str,
    answers: &[Answer<'_>],
) -> bool {
    let (Some(level), Some(level_matches)) = (command_at(cmd, path), matches_at(matches, path))
    else {
        return false;
    };
    let Some(arg) = level.get_arguments().find(|arg| arg.get_id() == id) else {
        return false;
    };
    let answered = |other: &Id| {
        answers.iter().any(|answer| {
            answer.prompted_arg.subcommand_path == path && answer.prompted_arg.attr.arg_id == other
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

/// The command at `path` passed through `edit`; unchanged when the path is missing.
pub(super) fn with_command_at<F>(cmd: Command, path: &[&str], edit: F) -> Command
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

/// Set each answer as its arg's default values. No values leave the arg with no default, so
/// an emptied list drops the declared default.
fn inject<'prompted_arg>(
    cmd: Command,
    answers: impl IntoIterator<Item = (&'prompted_arg PromptedArg, &'prompted_arg [String])>,
) -> Command {
    answers.into_iter().fold(cmd, |cmd, (prompted_arg, values)| {
        inject_at(cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id, values)
    })
}

/// Set `values` as the default values of arg `id` of the command at `path`.
fn inject_at(cmd: Command, path: &[&str], id: &str, values: &[String]) -> Command {
    // A default value is no explicit value to clap, so the rules that require the arg would
    // still miss it.
    with_arg_at(unrequire(cmd, path, id), path, id, |arg| {
        arg.default_values(values.iter().cloned())
    })
}

/// `question` with each `{id}` replaced by arg `id`'s values from `values_of`, joined with `, `,
/// and `{{` and `}}` by single braces. A `{id}` naming no arg stays as written.
fn interpolate(question: &str, values_of: impl Fn(&str) -> Option<Vec<String>>) -> String {
    let mut interpolated = String::with_capacity(question.len());
    let mut rest = question;
    while let Some(brace) = rest.find(['{', '}']) {
        let (text, tail) = rest.split_at(brace);
        interpolated.push_str(text);
        let escaped = tail.strip_prefix("{{").map(|after| ('{', after));
        if let Some((brace, after)) =
            escaped.or_else(|| tail.strip_prefix("}}").map(|after| ('}', after)))
        {
            interpolated.push(brace);
            rest = after;
            continue;
        }
        let placeholder = tail.strip_prefix('{').and_then(|inner| inner.split_once('}'));
        if let Some((values, after)) =
            placeholder.and_then(|(id, after)| values_of(id).map(|values| (values, after)))
        {
            interpolated.push_str(&values.join(", "));
            rest = after;
            continue;
        }
        // A lone brace, or a `{id}` naming no arg: keep the brace as written.
        let (brace, after) = tail.split_at(1);
        interpolated.push_str(brace);
        rest = after;
    }
    interpolated.push_str(rest);
    interpolated
}

pub(super) fn matches_at<'matches>(
    matches: &'matches ArgMatches,
    path: &[&str],
) -> Option<&'matches ArgMatches> {
    path.iter().try_fold(matches, |level, name| {
        level.subcommand().filter(|(taken, _)| taken == name).map(|(_, sub)| sub)
    })
}

/// The prompted args on the path the user took that got no value or only their default, or `-`
/// to be asked for, with the default's values, split at the arg's value delimiter as clap splits
/// them. One whose `default_value_if` fired is left to the arg that fired it: the conditional
/// default beats an injected answer, so asking would drop the answer.
fn pending<'prompted_arg>(
    cmd: &Command,
    matches: &ArgMatches,
    prompted_args: &'prompted_arg [PromptedArg],
    dashed: &Dashed<'_>,
) -> Vec<(&'prompted_arg PromptedArg, Vec<String>)> {
    // Built, so a flag's action has filled in its implicit default (`false` for `SetTrue`).
    let mut built = cmd.clone();
    built.build();
    prompted_args
        .iter()
        .filter_map(|prompted_arg| {
            let (path, id) = (&prompted_arg.subcommand_path, prompted_arg.attr.arg_id);
            // Passed `-`, it has a value, but one the user asked to be asked for.
            if dashed.asks(path, id) {
                return Some((prompted_arg, Vec::new()));
            }
            unfilled(&built, matches, path, id).map(|current| (prompted_arg, current))
        })
        .collect()
}

/// The values of arg `id` of the command at `path`, when it is on the path the user took and got
/// no value or only its default; see [`pending`]. `built` must be built.
pub(super) fn unfilled(
    built: &Command,
    matches: &ArgMatches,
    path: &[&str],
    id: &str,
) -> Option<Vec<String>> {
    let arg = arg_at(built, path, id)?;
    let level = matches_at(matches, path)?;
    match level.value_source(id) {
        None if default_values(arg).next().is_none() => Some(Vec::new()),
        Some(ValueSource::DefaultValue) => {
            let current = raw_values(level, id);
            current.iter().map(String::as_str).eq(default_values(arg)).then_some(current)
        }
        None | Some(_) => None,
    }
}

pub(super) fn raw_values(matches: &ArgMatches, id: &str) -> Vec<String> {
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

/// Make each of `prompted_args` optional, so a missing one does not stop a parse.
pub(super) fn relax<'prompted_arg>(
    cmd: Command,
    prompted_args: impl IntoIterator<Item = &'prompted_arg PromptedArg>,
) -> Command {
    prompted_args.into_iter().fold(cmd, |cmd, prompted_arg| {
        unrequire(cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
    })
}

/// The first line of clap's message, without its `error: ` prefix. clap quotes the rejected
/// `values`, so a multi-line one is shortened to its first line first, keeping the reason after it.
fn summary(err: &clap::Error, values: &[String]) -> String {
    let mut rendered = err.render().to_string();
    for value in values {
        if let Some((first_line, _)) = value.split_once('\n') {
            rendered = rendered.replace(value.as_str(), &format!("{first_line}\u{2026}"));
        }
    }
    let line = rendered.lines().next().unwrap_or_default();
    line.strip_prefix("error: ").unwrap_or(line).to_owned()
}

/// Drop the rules that make arg `id` of the command at `path` required: its `required`, its
/// `required_unless_present`, and the `required` of the groups it belongs to. clap can't clear
/// `required_unless_present_all` or `required_if_eq`, so those stay.
pub(super) fn unrequire(cmd: Command, path: &[&str], id: &str) -> Command {
    with_command_at(cmd, path, |level| {
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
pub(super) fn with_arg_at<F>(cmd: Command, path: &[&str], id: &str, edit: F) -> Command
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

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, io};

    use clap::{CommandFactory as _, Parser};
    use rstest::rstest;
    use secrecy::SecretString;

    use super::parse;
    use crate::{
        __private::{DashAttr, PromptAttr, PromptPurpose, PromptedArgTree, StdinAttr},
        Messages,
        interactive::{
            Backend, ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt,
            TextPrompt,
        },
        os::stdin::{ReadWhen, Stdin},
    };

    /// `#[couture(prompt(dash), stdin)]` on `--message`.
    static ASKED_OR_READ: PromptedArgTree = PromptedArgTree {
        branches: &[],
        own_args: &[PromptAttr {
            arg_id: "message",
            dash: Some(DashAttr::new::<String>()),
            editor: None,
            purpose: PromptPurpose::Value,
            question: None,
            secret: None,
        }],
        stdin_arg: Some(StdinAttr { arg_id: "message", dash: Some(DashAttr::new::<String>()) }),
    };

    /// Answers every text prompt with `"asked"`, when available.
    struct Answering {
        available: bool,
    }

    #[derive(Parser)]
    struct Draft {
        #[arg(long)]
        message: Option<String>,
    }

    /// Stdin holding `"read"`, a terminal or not.
    struct FakeStdin {
        terminal: bool,
    }

    impl Backend for Answering {
        fn confirm(&self, _prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
            Err(PromptError::NotATerminal)
        }

        fn is_available(&self) -> bool {
            self.available
        }

        fn multi_select(
            &self,
            _prompt: &MultiSelectPrompt<'_>,
        ) -> Result<Vec<String>, PromptError> {
            Err(PromptError::NotATerminal)
        }

        fn secret(&self, _prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
            Err(PromptError::NotATerminal)
        }

        fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
            Err(PromptError::NotATerminal)
        }

        fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
            Ok(Some("asked".to_owned()))
        }
    }

    impl Stdin for FakeStdin {
        fn is_terminal(&self) -> bool {
            self.terminal
        }

        fn read(&self, when: ReadWhen) -> io::Result<Option<Vec<u8>>> {
            Ok((when == ReadWhen::Always || !self.terminal).then(|| b"read\n".to_vec()))
        }
    }

    #[rstest]
    #[case::asks_at_a_terminal(true, true, "asked")]
    #[case::reads_stdin_that_is_no_terminal(false, true, "read")]
    #[case::reads_a_terminal_it_cannot_ask_on(true, false, "read")]
    fn a_dash_that_may_ask_or_read(
        #[case] terminal: bool,
        #[case] available: bool,
        #[case] message: &str,
    ) {
        let argv: Vec<OsString> = ["draft", "--message", "-"].map(OsString::from).into();
        let draft: Result<Draft, _> = parse(
            Draft::command(),
            &ASKED_OR_READ,
            &Messages::DEFAULT,
            &argv,
            &Answering { available },
            &FakeStdin { terminal },
        );
        assert_eq!(draft.ok().and_then(|draft| draft.message).as_deref(), Some(message));
    }
}
