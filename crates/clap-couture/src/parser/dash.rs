//! `prompt(dash)` and `stdin(dash)`: `-` as an arg's value stands for what is asked for it, or
//! read from stdin.
//!
//! clap parses `-` like any value. A dash arg's value parser swaps it for what the arg's
//! [`DashSlot`] holds, and runs the arg's own value parser on that, so clap checks and converts it
//! like a typed value. Until the slot is filled, [`CheckingValueParser`] lets `-` through; the
//! final parse's [`TypedDashValueParser`] turns it into the field's type. An arg with both dashes
//! asks when it can, at a terminal, and reads stdin otherwise, as when it is piped.

use alloc::sync::Arc;
use core::marker::PhantomData;
use std::{
    ffi::{OsStr, OsString},
    sync::{Mutex, MutexGuard, PoisonError},
};

use clap::{
    Arg, ArgMatches, Command,
    builder::{PossibleValue, TypedValueParser, ValueParser},
    error::ErrorKind,
    parser::ValueSource,
};

use super::{
    Failure,
    prompting::{arg_at, matches_at, raw_values, with_arg_at},
};
use crate::interactive::{DashAttr, PromptedArg, StdinArg};

/// The id of the one arg of [`DashValueParser::parse_alone`]'s command.
const VALUE: &str = "value";

/// One arg that takes `-`, from `prompt(dash)`, `stdin(dash)` or both.
pub(super) struct DashArg {
    /// The clap id of the arg.
    arg_id: &'static str,
    /// `prompt(dash)`: `-` may ask for it.
    asks: bool,
    /// Makes its value parser for the final parse.
    attr: DashAttr,
    /// `stdin(dash)`: `-` may read stdin into it.
    reads: bool,
    /// What `-` stands for, once asked or read.
    slot: DashSlot,
    /// Subcommand names from the root, empty for a root arg.
    subcommand_path: Vec<&'static str>,
}

/// Every arg of one parse that takes `-`.
pub(super) struct DashArgs(Vec<DashArg>);

/// The dash args the user passed `-` to.
#[derive(Default)]
pub(super) struct Dashed<'dash_args> {
    /// Those to ask for.
    asked: Vec<&'dash_args DashArg>,
    /// Those to read stdin into.
    read: Vec<&'dash_args DashArg>,
}

/// What `-` stands for in one dash arg: an answer, stdin's text, or a secret's vault placeholder.
/// Shared by every clone of the arg's value parser.
#[derive(Clone, Debug, Default)]
pub(crate) struct DashSlot(Arc<Mutex<Option<String>>>);

/// The arg's own value parser, and what stands in for `-`.
#[derive(Clone)]
pub(crate) struct DashValueParser {
    inner: ValueParser,
    slot: DashSlot,
}

/// A dash arg's value parser before the final parse: lets an unfilled `-` through, and checks
/// anything else with the arg's own value parser.
#[derive(Clone)]
struct CheckingValueParser(DashValueParser);

/// A dash arg's value parser in the final parse: `-` becomes what fills it, as the field's `T`.
#[derive(Clone)]
pub(crate) struct TypedDashValueParser<T> {
    dash: DashValueParser,
    field: PhantomData<fn() -> T>,
}

impl DashArg {
    /// Check whether this is arg `id` of the command at `path`.
    fn is(&self, path: &[&str], id: &str) -> bool {
        self.subcommand_path == path && self.arg_id == id
    }

    /// Check whether the user passed this arg `-`, in `matches` of a parse whose dash args let it
    /// through.
    fn passed_dash(&self, matches: &ArgMatches) -> bool {
        matches_at(matches, &self.subcommand_path).is_some_and(|level| {
            level.value_source(self.arg_id) == Some(ValueSource::CommandLine)
                && raw_values(level, self.arg_id) == ["-"]
        })
    }

    /// The error for its `-` with no terminal to ask on.
    pub(super) fn unaskable(&self, cmd: &Command) -> Failure {
        let mut built = cmd.clone();
        built.build();
        let arg = arg_at(&built, &self.subcommand_path, self.arg_id)
            .map_or_else(|| self.arg_id.to_owned(), ToString::to_string);
        let message = format!("{arg} got `-` to be asked for, but there is no terminal to ask on");
        Failure::Clap(
            clap::Error::raw(ErrorKind::ValueValidation, message).format(&mut cmd.clone()),
        )
    }

    /// `cmd` with this arg's value parser passed through `wrap`.
    fn wrapped(&self, cmd: Command, wrap: impl FnOnce(DashValueParser) -> ValueParser) -> Command {
        with_arg_at(cmd, &self.subcommand_path, self.arg_id, |arg| {
            let dash =
                DashValueParser { inner: arg.get_value_parser().clone(), slot: self.slot.clone() };
            arg.value_parser(wrap(dash))
        })
    }
}

impl DashArgs {
    /// `cmd` with each dash arg letting an unfilled `-` through.
    pub(super) fn checking(&self, cmd: Command) -> Command {
        self.0.iter().fold(cmd, |cmd, dash_arg| {
            dash_arg.wrapped(cmd, |dash| ValueParser::new(CheckingValueParser(dash)))
        })
    }

    /// Fill the slot of `prompted_arg`, if it is a dash arg, with its answer `values`.
    pub(super) fn fill(&self, prompted_arg: &PromptedArg, values: &[String]) {
        let (path, id) = (&prompted_arg.subcommand_path, prompted_arg.attr.arg_id);
        if let (Some(dash_arg), Some(value)) =
            (self.0.iter().find(|dash_arg| dash_arg.is(path, id)), values.first())
        {
            dash_arg.slot.fill(value.clone());
        }
    }

    /// Check whether no arg takes `-`.
    pub(super) const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The dash args of `prompted_args` and `stdin_args`, each once.
    pub(super) fn of(prompted_args: &[PromptedArg], stdin_args: &[StdinArg]) -> Self {
        let mut dash_args: Vec<DashArg> = Vec::new();
        let asking = prompted_args.iter().filter_map(|prompted_arg| {
            let attr = prompted_arg.attr.dash?;
            Some((&prompted_arg.subcommand_path, prompted_arg.attr.arg_id, attr, true))
        });
        let reading = stdin_args.iter().filter_map(|stdin_arg| {
            let attr = stdin_arg.attr.dash?;
            Some((&stdin_arg.subcommand_path, stdin_arg.attr.arg_id, attr, false))
        });
        for (path, arg_id, attr, asks) in asking.chain(reading) {
            match dash_args.iter_mut().find(|dash_arg| dash_arg.is(path, arg_id)) {
                Some(dash_arg) => dash_arg.reads = true,
                None => dash_args.push(DashArg {
                    asks,
                    arg_id,
                    attr,
                    reads: !asks,
                    slot: DashSlot::default(),
                    subcommand_path: path.clone(),
                }),
            }
        }
        Self(dash_args)
    }

    /// The dash args passed `-` in `matches`, of a parse through [`Self::checking`]. One with
    /// both dashes is asked for when `can_ask`, and reads stdin otherwise.
    pub(super) fn passed(&self, matches: &ArgMatches, can_ask: bool) -> Dashed<'_> {
        let (asked, read) = self
            .0
            .iter()
            .filter(|dash_arg| dash_arg.passed_dash(matches))
            .partition(|dash_arg| dash_arg.asks && (can_ask || !dash_arg.reads));
        Dashed { asked, read }
    }

    /// `cmd` with each dash arg's value parser giving its field's type, `-` filled from its slot.
    pub(super) fn typed(&self, cmd: Command) -> Command {
        self.0.iter().fold(cmd, |cmd, dash_arg| {
            dash_arg.wrapped(cmd, |dash| dash_arg.attr.typed_parser(dash))
        })
    }
}

impl Dashed<'_> {
    /// Check whether arg `id` of the command at `path` was passed `-` to be asked for.
    pub(super) fn asks(&self, path: &[&str], id: &str) -> bool {
        self.asked.iter().any(|dash_arg| dash_arg.is(path, id))
    }

    /// Fill the slot of the arg read into, arg `id` of the command at `path`, with `text`.
    /// Returns whether it was one.
    pub(super) fn fill_read(&self, path: &[&str], id: &str, text: String) -> bool {
        let read = self.read.iter().find(|dash_arg| dash_arg.is(path, id));
        if let Some(dash_arg) = read {
            dash_arg.slot.fill(text);
        }
        read.is_some()
    }

    /// The first arg passed `-` to be asked for, if any.
    pub(super) fn first_asked(&self) -> Option<&DashArg> {
        self.asked.first().copied()
    }

    /// Check whether arg `id` of the command at `path` was passed `-` to read stdin.
    pub(super) fn reads(&self, path: &[&str], id: &str) -> bool {
        self.read.iter().any(|dash_arg| dash_arg.is(path, id))
    }

    /// What fills the `-` of arg `id` of the command at `path`, if it was passed one and it is
    /// filled.
    pub(super) fn text(&self, path: &[&str], id: &str) -> Option<String> {
        self.asked
            .iter()
            .chain(&self.read)
            .find(|dash_arg| dash_arg.is(path, id))
            .and_then(|dash_arg| dash_arg.slot.text().clone())
    }
}

impl DashSlot {
    fn fill(&self, text: String) {
        *self.text() = Some(text);
    }

    // Nothing panics while holding the lock, so a poisoned one still holds whole text.
    fn text(&self) -> MutexGuard<'_, Option<String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl DashValueParser {
    /// `value` from `source`, with `-` from the command line swapped for the slot's text, run
    /// through the arg's own value parser. `None` for a `-` the slot can't fill yet.
    ///
    /// # Errors
    /// The arg's value parser rejected it.
    fn parse(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
        source: ValueSource,
    ) -> Result<Option<ArgMatches>, clap::Error> {
        if source == ValueSource::CommandLine && value == "-" {
            // The slot's text is no command-line value: a secret's placeholder resolves only
            // as a default.
            let Some(text) = self.slot.text().clone() else { return Ok(None) };
            return self
                .parse_alone(cmd, arg, OsStr::new(&text), ValueSource::DefaultValue)
                .map(Some);
        }
        self.parse_alone(cmd, arg, value, source).map(Some)
    }

    /// `value` parsed by the arg's own value parser, alone in a command named like `arg`, so
    /// clap's message names it. clap gives no other way to run a [`ValueParser`].
    ///
    /// # Errors
    /// The arg's value parser rejected it.
    fn parse_alone(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
        source: ValueSource,
    ) -> Result<ArgMatches, clap::Error> {
        let named = Arg::new(VALUE)
            .value_parser(self.inner.clone())
            .allow_hyphen_values(true)
            .value_names(arg.and_then(Arg::get_value_names).unwrap_or_default().to_vec());
        let named = match arg.map(|arg| (arg.get_long(), arg.get_short())) {
            Some((Some(long), _)) => named.long(long.to_owned()),
            Some((None, Some(short))) => named.short(short),
            _ => named,
        };
        let from_command_line = source == ValueSource::CommandLine;
        let named = if from_command_line { named } else { named.default_value(value.to_owned()) };
        let words: Vec<OsString> = match (from_command_line, named.get_long(), named.get_short()) {
            (false, ..) => Vec::new(),
            (true, Some(long), _) => {
                let mut word = OsString::from(format!("--{long}="));
                word.push(value);
                vec![word]
            }
            (true, None, Some(short)) => vec![format!("-{short}").into(), value.to_owned()],
            (true, None, None) => vec!["--".into(), value.to_owned()],
        };
        Command::new(cmd.get_name().to_owned())
            .no_binary_name(true)
            .arg(named)
            .try_get_matches_from(words)
            .map_err(|err| err.format(&mut cmd.clone()))
    }
}

impl TypedValueParser for CheckingValueParser {
    type Value = OsString;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<OsString, clap::Error> {
        self.parse_ref_(cmd, arg, value, ValueSource::CommandLine)
    }

    fn parse_ref_(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
        source: ValueSource,
    ) -> Result<OsString, clap::Error> {
        self.0.parse(cmd, arg, value, source).map(|_| value.to_owned())
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        self.0.inner.possible_values()
    }
}

impl<T> TypedDashValueParser<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// `dash` as a value parser giving `T`.
    pub(crate) fn value_parser(dash: DashValueParser) -> ValueParser {
        ValueParser::new(Self { dash, field: PhantomData })
    }
}

impl<T> TypedValueParser for TypedDashValueParser<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Value = T;

    fn parse_ref(&self, cmd: &Command, arg: Option<&Arg>, value: &OsStr) -> Result<T, clap::Error> {
        self.parse_ref_(cmd, arg, value, ValueSource::CommandLine)
    }

    fn parse_ref_(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
        source: ValueSource,
    ) -> Result<T, clap::Error> {
        let unfilled = || {
            let arg = arg.map_or_else(|| "an arg".to_owned(), ToString::to_string);
            let message = format!("{arg} got `-`, but nothing was asked or read for it");
            clap::Error::raw(ErrorKind::ValueValidation, message).format(&mut cmd.clone())
        };
        let mut matches = self.dash.parse(cmd, arg, value, source)?.ok_or_else(unfilled)?;
        matches.remove_one::<T>(VALUE).ok_or_else(unfilled)
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        self.dash.inner.possible_values()
    }
}
