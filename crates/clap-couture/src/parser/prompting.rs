//! Parse, ask for the prompted args that got no value, then parse again with the answers.
//!
//! Every answer goes back into the command as its arg's `default_values`, so clap converts and
//! validates it exactly like a typed value. Each answer is checked with a trial parse as soon as
//! it is given: the previous trial passed, so a failure can only come from the new answer, which
//! is then asked again with clap's message. For an arg asked one value at a time, each value is
//! an answer of its own, checked on top of the values before it. clap holds no default to
//! `num_args`, so the answer is held to it here, as one time the arg is passed.
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
    Arg, ArgAction, ArgMatches, Command, FromArgMatches, Id,
    builder::{PossibleValue, Resettable, ValueRange},
    error::{ContextKind, ContextValue, ErrorKind},
    parser::ValueSource,
};

use super::{
    ParseError,
    dash::{DashArgs, Dashed},
    secrets::{self, SecretRequirement as _, Vault},
    stdin::StdinArgs,
};
use crate::{
    Messages,
    help_tree::DeferredHelp,
    interactive::{
        Backend, CommandStyles, ConfirmPrompt, MultiSelectPrompt, PromptError, PromptPurpose,
        PromptedArg, PromptedArgTree, SecretPrompt, SelectPrompt, TextPrompt,
    },
    os::{signal::Signal, stdin::Stdin},
    render::CommandRenderExt as _,
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

/// [`Command`], asked whether its args conflict.
trait Conflicting {
    /// Check whether clap refuses `together` passed at once. Parses a `--{id}` flag per arg, with
    /// the conflicts and exclusivity of those in `together`, beside `self`'s groups as declared:
    /// clap has no getter for a group's own conflicts. `inherited` are globals a group may name.
    fn refuses_together(&self, together: &[&str], inherited: &[&Arg]) -> bool;
}

impl Conflicting for Command {
    fn refuses_together(&self, together: &[&str], inherited: &[&Arg]) -> bool {
        let mut args: Vec<&Arg> = self.get_arguments().collect();
        for global in inherited {
            if !args.iter().any(|arg| arg.get_id() == global.get_id()) {
                args.push(global);
            }
        }
        let flags = args.iter().map(|arg| {
            let flag = Arg::new(arg.get_id().clone())
                .long(arg.get_id().clone())
                .action(ArgAction::SetTrue)
                .exclusive(arg.is_exclusive_set());
            if !together.iter().any(|id| arg.get_id() == id) {
                return flag;
            }
            let conflicts: Vec<Id> = self
                .get_arg_conflicts_with(arg)
                .into_iter()
                .map(Arg::get_id)
                .filter(|conflict| args.iter().any(|arg| arg.get_id() == *conflict))
                .cloned()
                .collect();
            flag.conflicts_with_all(conflicts)
        });
        Self::new("conflicts")
            .no_binary_name(true)
            .disable_help_flag(true)
            .disable_version_flag(true)
            .args(flags)
            .groups(self.get_groups().cloned())
            .try_get_matches_from(together.iter().map(|id| format!("--{id}")))
            .is_err_and(|err| err.kind() == ErrorKind::ArgumentConflict)
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
    /// argv parsed before any answer; a prompted arg with no value from the user in it was left
    /// out.
    passed: &'run ArgMatches,
    prompted_args: &'run [PromptedArg],
    /// Every secret arg, asked or not; a confirmation masks their values.
    secret_args: &'run [PromptedArg],
    /// Holds the secret answers; `cmd` resolves their placeholders from it.
    vault: &'run Vault,
}

/// How many values one answer to an arg may hold: its `num_args`, as for one time it is passed.
#[derive(Clone, Copy)]
struct ValueCount<'arg> {
    arg: &'arg Arg,
    range: ValueRange,
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
        let count = ValueCount::of(arg);
        if !takes_many(arg) {
            return self.until_accepted(answers, prompted_arg, count, || {
                let value = self.ask(arg, &question, &options, current.first(), optional)?;
                Ok(value.map(|value| vec![value]))
            });
        }
        if options.is_empty() {
            return self
                .each_value(&question, prompted_arg, current, answers, optional, count)
                .map(Some);
        }
        self.until_accepted(answers, prompted_arg, count, || {
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
    /// the editor's `initial` text. A text arg keeps the newline the answer ends in. An empty
    /// answer to a required arg aborts, as git aborts on an empty commit message, and so does one
    /// to an arg with a default, which an empty answer would quietly bring back.
    #[cfg(feature = "interactive-editor")]
    fn ask_in_editor(
        &self,
        asked: &Asked<'_>,
        editor: &EditorAttr,
        rejected: Option<&Rejected>,
    ) -> Result<Option<String>, PromptError> {
        use core::any::TypeId;

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
            unreadable: &self.messages.editor_unreadable.get(),
            waiting: &self.messages.editor_waiting.get(),
        })?;
        if answer.is_none() && !optional {
            self.backend.report(&self.messages.editor_empty.get(), styles)?;
            return Err(PromptError::Cancelled);
        }
        // The file's last newline is part of a text, not of a number, a flag or a path.
        let value_type = arg.get_value_parser().type_id();
        let takes_text = [TypeId::of::<String>(), TypeId::of::<OsString>()]
            .iter()
            .any(|text| value_type == *text);
        Ok(answer.map(|mut answer| {
            if !takes_text && answer.ends_with('\n') {
                answer.pop();
            }
            answer
        }))
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
            self.relaxed(self.prompted_args),
            answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice())),
        )
        .try_get_matches_from(self.argv);
        let question = match answered
            .as_ref()
            .ok()
            .and_then(|answered| matches_at(answered, &prompted_arg.subcommand_path))
        {
            Some(level) => interpolate(&question, |id| {
                let path = &prompted_arg.subcommand_path;
                let values = self
                    .dashed
                    .text(path, id)
                    .map(|text| vec![text])
                    .or_else(|| level.try_get_raw(id).ok().map(|_| raw_values(level, id)))?;
                if self.is_secret(path, id) {
                    return Some(values.iter().map(|_| secrets::MASKED.to_owned()).collect());
                }
                Some(values)
            }),
            None => question,
        };
        self.backend.confirm(&ConfirmPrompt {
            default: false,
            question: &question,
            styles: CommandStyles(self.cmd.get_styles()),
        })
    }

    /// Ask for one value at a time until an empty answer or `count`'s most, each starting out as
    /// the default's value at its position. Only a required arg's first value may not be empty,
    /// and an empty answer short of `count`'s least is asked again with clap's message.
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
        count: Option<ValueCount<'_>>,
    ) -> Result<Vec<String>, PromptError> {
        let finish_hint = self.messages.finish_hint.get();
        let mut values = Vec::new();
        loop {
            if count.is_some_and(|count| !count.has_room(values.len())) {
                return Ok(values);
            }
            let short = count.and_then(|count| count.check(&values).err());
            let optional = !values.is_empty() || optional;
            let Some(value) = self.backend.text(&TextPrompt {
                default: None,
                hint: (optional && short.is_none()).then_some(&*finish_hint),
                initial: current.get(values.len()).map(String::as_str),
                optional,
                question,
                styles: CommandStyles(self.cmd.get_styles()),
            })?
            else {
                let Some(err) = short else { return Ok(values) };
                self.backend
                    .report(&summary(&err, &values), CommandStyles(self.cmd.get_styles()))?;
                continue;
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

    /// Check whether arg `id` at `path` is a secret, its own or a global one from a level above.
    fn is_secret(&self, path: &[&str], id: &str) -> bool {
        self.secret_args.iter().any(|secret_arg| {
            secret_arg.attr.arg_id == id && path.starts_with(&secret_arg.subcommand_path)
        })
    }

    /// The `= ...` override, else the arg's help text, else its id, rendered.
    fn question(&self, prompted_arg: &PromptedArg) -> String {
        let question = prompted_arg.attr.question.as_ref().map_or_else(
            || {
                arg_at(self.cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
                    .and_then(Arg::get_help)
                    .map_or_else(|| prompted_arg.attr.arg_id.to_owned(), ToString::to_string)
            },
            |question| question.get().into_owned(),
        );
        self.cmd.render_question(&question)
    }

    /// `self.cmd` with `prompted_args` relaxed, and those of them the user left out excused.
    fn relaxed<'prompted_arg>(
        &self,
        prompted_args: impl IntoIterator<Item = &'prompted_arg PromptedArg> + Clone,
    ) -> Command {
        excuse(relax(self.cmd.clone(), prompted_args.clone()), self.passed, prompted_args)
    }

    /// Check whether leaving the arg being asked unset fails the parse: it is `required`, or
    /// required through `required_unless_present`, a required group or a passed arg's `requires`.
    /// The accepted answers go in, and only the args still to be asked, `later`, stay relaxed, so
    /// a group another of them can still satisfy doesn't make this one required.
    fn required(&self, answers: &[Answer<'_>], later: &[&PromptedArg]) -> bool {
        let accepted =
            answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice()));
        let answered = answers.iter().map(|answer| answer.prompted_arg);
        let cmd =
            excuse(inject(self.relaxed(later.iter().copied()), accepted), self.passed, answered);
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
        let cmd = inject(self.relaxed(self.prompted_args), with_candidate);
        cmd.try_get_matches_from(self.argv).map(drop)
    }

    /// Call `ask` until clap accepts its answer and it fits `count`, showing clap's complaint
    /// before each retry.
    fn until_accepted(
        &self,
        answers: &[Answer<'_>],
        prompted_arg: &PromptedArg,
        count: Option<ValueCount<'_>>,
        mut ask: impl FnMut() -> Result<Option<Vec<String>>, PromptError>,
    ) -> Result<Option<Vec<String>>, PromptError> {
        loop {
            let Some(values) = ask()? else { return Ok(None) };
            let fits = count.map_or(Ok(()), |count| count.check(&values));
            match fits.and_then(|()| self.trial(answers, prompted_arg, &values)) {
                Ok(()) => return Ok(Some(values)),
                Err(err) => {
                    let error = summary(&err, &values);
                    self.backend.report(&error, CommandStyles(self.cmd.get_styles()))?;
                }
            }
        }
    }
}

impl<'arg> ValueCount<'arg> {
    /// Check `values` as clap checks them typed after the arg once; no values leave it unset.
    fn check(self, values: &[String]) -> Result<(), clap::Error> {
        let (min, max) = (self.range.min_values(), self.range.max_values());
        if let Some(surplus) = values.get(max) {
            return Err(self.error(ErrorKind::TooManyValues, [(
                ContextKind::InvalidValue,
                ContextValue::String(surplus.clone()),
            )]));
        }
        if values.is_empty() || values.len() >= min {
            return Ok(());
        }
        let number =
            |count: usize| ContextValue::Number(isize::try_from(count).unwrap_or(isize::MAX));
        let actual = (ContextKind::ActualNumValues, number(values.len()));
        // clap's own wording: an exact count when there is one, else the least it takes.
        Err(if min == max {
            self.error(ErrorKind::WrongNumberOfValues, [
                actual,
                (ContextKind::ExpectedNumValues, number(min)),
            ])
        } else {
            self.error(ErrorKind::TooFewValues, [actual, (ContextKind::MinValues, number(min))])
        })
    }

    /// clap's `kind` of error about the arg, with `context` filled in.
    fn error(
        self,
        kind: ErrorKind,
        context: impl IntoIterator<Item = (ContextKind, ContextValue)>,
    ) -> clap::Error {
        let mut err = clap::Error::new(kind);
        err.insert(ContextKind::InvalidArg, ContextValue::String(self.arg.to_string()));
        for (kind, value) in context {
            err.insert(kind, value);
        }
        err
    }

    /// Check whether `count` values leave room for another.
    fn has_room(self, count: usize) -> bool {
        count < self.range.max_values()
    }

    /// `arg`'s `num_args`; `None` when it takes one value each time and may be passed again, as a
    /// plain `Vec` does, so any number of values is one it could be typed with.
    fn of(arg: &'arg Arg) -> Option<Self> {
        let range = arg.get_num_args()?;
        let repeats_single_values =
            matches!(arg.get_action(), ArgAction::Append) && range.max_values() <= 1;
        (!repeats_single_values).then_some(Self { arg, range })
    }
}

/// Parse `argv` against `cmd`, filling its stdin arg from `stdin`, asking `backend` for the
/// prompted args that got no value, and standing in for each `-` a dash arg got what was asked or
/// read for it. The help clap prints is `help`.
pub(super) fn parse<T>(
    cmd: Command,
    prompted_arg_tree: &'static PromptedArgTree,
    messages: &Messages,
    argv: &[OsString],
    backend: &dyn Backend,
    stdin: &impl Stdin,
    help: &DeferredHelp,
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
        return parse_unprompted(cmd, &prompted_args, argv, help);
    }
    // Re-guarded with the vault this parse's answers go into; trial and final parses share it.
    let vault = Vault::default();
    let mut cmd = secrets::guard(cmd, &prompted_args, &vault, &messages.environment.get());
    let relaxed = stdin_args.unrequired(relax(dash_args.checking(cmd.clone()), &prompted_args));
    // Prompts read the terminal, so a `-` that may ask or read stdin reads it when it is no
    // terminal, as when piped.
    let can_ask = stdin.is_terminal() && backend.is_available();
    // Help, usage and errors come from clap, with `-` let through.
    let Ok(passed) = parse_relaxed(relaxed, &prompted_args, argv) else {
        return super::parse(dash_args.checking(cmd), argv, help);
    };
    let dashed = dash_args.passed(&passed, can_ask);
    // `-` is the only way to ask a `prompt(dash)` arg.
    prompted_args.retain(|prompted_arg| {
        prompted_arg.attr.dash.is_none()
            || dashed.asks(&prompted_arg.subcommand_path, prompted_arg.attr.arg_id)
    });
    if let Some(asked) = dashed.first_asked().filter(|_| !backend.is_available()) {
        return Err(asked.unaskable(&cmd));
    }
    let checking = dash_args.checking(cmd.clone());
    if let Some(stdin_value) = stdin_args.read(&checking, &prompted_args, &dashed, argv, stdin)? {
        cmd = stdin_value.fill(cmd, &mut prompted_args, &dashed, &vault);
    }
    if prompted_args.is_empty() || !backend.is_available() {
        return parse_unprompted(dash_args.typed(cmd), &prompted_args, argv, help);
    }
    let checking = dash_args.checking(cmd.clone());
    // Taken before any were dropped from `prompted_args`: a secret read from stdin is masked too.
    let secret_args: Vec<PromptedArg> = prompted_arg_tree
        .prompted_args()
        .into_iter()
        .filter(|prompted_arg| prompted_arg.attr.secret.is_some())
        .collect();
    // Help, usage and errors come from the command the user sees, not the relaxed one.
    let relaxed = excuse(relax(checking.clone(), &prompted_args), &passed, &prompted_args);
    let Ok(matches) = relaxed.try_get_matches_from(argv) else {
        return super::parse(checking, argv, help);
    };

    let session = Session {
        argv,
        backend,
        cmd: &checking,
        dash_args: &dash_args,
        dashed: &dashed,
        messages,
        passed: &passed,
        prompted_args: &prompted_args,
        secret_args: &secret_args,
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
                return parse_unprompted(dash_args.typed(cmd), &prompted_args, argv, help);
            }
        }
    }
    let cmd_with_answers = excuse(
        inject(
            cmd,
            answers.iter().map(|answer| (answer.prompted_arg, answer.injected_values.as_slice())),
        ),
        &passed,
        answers.iter().map(|answer| answer.prompted_arg),
    );
    super::parse(dash_args.typed(cmd_with_answers), argv, help)
}

/// Parse as clap would, without asking. A confirmation left unanswered fails the parse with
/// clap's error for a missing required arg, so only passing its flag gets past it. The help clap
/// prints is `help`.
fn parse_unprompted<T>(
    mut cmd: Command,
    prompted_args: &[PromptedArg],
    argv: &[OsString],
    help: &DeferredHelp,
) -> Result<T, ParseError>
where
    T: FromArgMatches,
{
    let confirmations: Vec<_> = prompted_args
        .iter()
        .filter(|prompted_arg| prompted_arg.attr.purpose == PromptPurpose::Confirmation)
        .collect();
    if confirmations.is_empty() {
        return super::parse(cmd, argv, help);
    }
    // Kept unbuilt: requiring an arg on the built command makes clap show help instead.
    let declared = cmd.clone();
    let mut matches = help.matches(&mut cmd, argv).map_err(ParseError::Clap)?;
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
        return super::parse(required, argv, help);
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
    if !level.get_arguments().any(|arg| arg.get_id() == id) {
        return false;
    }
    let answered = |other: &Id| {
        answers.iter().any(|answer| {
            answer.prompted_arg.subcommand_path == path && answer.prompted_arg.attr.arg_id == other
        })
    };
    let together: Vec<&str> = level
        .get_arguments()
        .map(Arg::get_id)
        .filter(|other| *other != id)
        .filter(|other| answered(other) || explicit(level_matches, other))
        .map(Id::as_str)
        .chain(iter::once(id))
        .collect();
    // Alone, it conflicts with nothing.
    if together.len() < 2 {
        return false;
    }
    let inherited: Vec<&Arg> = (0..path.len())
        .filter_map(|depth| command_at(cmd, path.get(..depth)?))
        .flat_map(Command::get_arguments)
        .filter(|arg| arg.is_global_set())
        .collect();
    level.refuses_together(&together, &inherited)
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

/// Let each of `prompted_args` the user left out in `passed` go missing though a passed arg
/// `requires` it: clap lets a required arg go missing beside a passed arg it conflicts with, so it
/// conflicts with every other arg of its command. Having no value from the user, it never trips
/// that conflict. A global one is left as is, as its conflicts would reach subcommands.
fn excuse<'prompted_arg>(
    cmd: Command,
    passed: &ArgMatches,
    prompted_args: impl IntoIterator<Item = &'prompted_arg PromptedArg>,
) -> Command {
    prompted_args.into_iter().fold(cmd, |cmd, prompted_arg| {
        let (path, id) = (prompted_arg.subcommand_path.as_slice(), prompted_arg.attr.arg_id);
        if matches_at(passed, path).is_some_and(|level| explicit(level, &Id::from(id))) {
            return cmd;
        }
        with_command_at(cmd, path, |level| {
            let others: Vec<Id> = level
                .get_arguments()
                .map(Arg::get_id)
                .filter(|other| *other != id)
                .cloned()
                .collect();
            with_arg_at(level, &[], id, |arg| {
                if arg.is_global_set() { arg } else { arg.conflicts_with_all(others) }
            })
        })
    })
}

/// Check whether the user passed `id`, on the command line or through its env var.
fn explicit(matches: &ArgMatches, id: &Id) -> bool {
    matches!(
        matches.value_source(id.as_str()),
        Some(ValueSource::CommandLine | ValueSource::EnvVariable)
    )
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
pub(super) fn inject_at(cmd: Command, path: &[&str], id: &str, values: &[String]) -> Command {
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
        None if arg.is_required_by_default() || default_values(arg).next().is_none() => {
            Some(Vec::new())
        }
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

/// Parse `argv` with `relaxed`, whose `prompted_args` are relaxed. When a passed arg requires one
/// left out, it is parsed again with those excused, found by a parse that sees past clap's errors.
fn parse_relaxed(
    relaxed: Command,
    prompted_args: &[PromptedArg],
    argv: &[OsString],
) -> Result<ArgMatches, clap::Error> {
    match relaxed.clone().try_get_matches_from(argv) {
        Err(err) if err.kind() == ErrorKind::MissingRequiredArgument => {
            let passed = relaxed.clone().ignore_errors(true).try_get_matches_from(argv)?;
            excuse(relaxed, &passed, prompted_args).try_get_matches_from(argv)
        }
        parsed => parsed,
    }
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
/// `required_unless_present`, a secret's required default, and the `required` of the groups it
/// belongs to. clap can't clear `required_unless_present_all` or `required_if_eq`, so those stay.
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
        let level = level.mut_arg(id, |arg| {
            arg.required(false)
                .required_unless_present(Resettable::Reset)
                .without_required_default()
        });
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

    use clap::{Arg, ArgGroup, Command, CommandFactory as _, Parser, error::ErrorKind};
    use rstest::rstest;
    use secrecy::SecretString;

    use super::{blocked, parse};
    use crate::{
        __private::{DashAttr, PromptAttr, PromptPurpose, PromptedArgTree, StdinAttr},
        Couture, Messages,
        command_shapes::Choices,
        help_tree::DeferredHelp,
        interactive::{
            Backend, ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt,
            TextPrompt,
        },
        os::stdin::{ReadWhen, Stdin},
    };

    /// The args of a generated conflict command, each a `--name <VALUE>` option.
    const IDS: [&str; 5] = ["alpha", "beta", "gamma", "delta", "omega"];

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
        stdin_arg: Some(StdinAttr {
            arg_id: "message",
            dash: DashAttr::new::<String>(),
            piped: false,
        }),
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

    impl Couture for Draft {}

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
            &DeferredHelp::of::<Draft>(Draft::command),
        );
        assert_eq!(draft.ok().and_then(|draft| draft.message).as_deref(), Some(message));
    }

    /// A command of options from [`IDS`], related by random conflicts, exclusivity and a group,
    /// and the same command without the group's own conflict.
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
                    only_by_group_conflict = only_by_group_conflict.saturating_add(usize::from(
                        refuses(&without_group_conflict) != clap_refuses,
                    ));
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
}
