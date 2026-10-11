//! `#[couture(stdin)]`: an arg passed `-` takes all of stdin, and a `stdin(piped)` one the user
//! left out takes what is piped into it.

use std::ffi::OsString;

use clap::{Command, error::ErrorKind};

use super::{
    ParseError,
    dash::Dashed,
    prompting::{arg_at, blocked, relax, unfilled, unrequire},
};
use crate::{
    interactive::{PromptedArg, PromptedArgTree, StdinArg},
    os::stdin::{ReadWhen, STDIN_LIMIT, Stdin},
};

/// The args marked `#[couture(stdin)]`, at most one on any one subcommand chain.
pub(super) struct StdinArgs(Vec<StdinArg>);

/// Stdin's text as the value of the arg that takes it, one trailing line ending dropped, so
/// `echo eu |` reads as `eu`.
pub(super) struct StdinValue<'stdin_arg> {
    pub(super) stdin_arg: &'stdin_arg StdinArg,
    pub(super) text: String,
}

impl StdinArgs {
    /// Check whether no arg takes stdin.
    pub(super) const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every stdin arg, in declaration order.
    pub(super) fn iter(&self) -> impl Iterator<Item = &StdinArg> {
        self.0.iter()
    }

    /// The stdin args of `prompted_arg_tree`.
    pub(super) fn of(prompted_arg_tree: &'static PromptedArgTree) -> Self {
        Self(prompted_arg_tree.stdin_args())
    }

    /// The stdin arg on the path the user took, if it got no value, and the text piped in for it.
    /// `None` when nothing is piped, or the arg conflicts with one the user passed. Stdin is read
    /// only once the rest of `argv` parses, so `--help` never waits on it. An arg passed `-` has
    /// a value, so no pipe fills it, and a `stdin` one takes only what `-` reads.
    ///
    /// # Errors
    /// - Reading stdin failed.
    /// - What was piped is not UTF-8.
    fn piped(
        &self,
        cmd: &Command,
        prompted_args: &[PromptedArg],
        argv: &[OsString],
        stdin: &impl Stdin,
    ) -> Result<Option<StdinValue<'_>>, ParseError> {
        let implicit: Vec<&StdinArg> =
            self.iter().filter(|stdin_arg| stdin_arg.attr.dash.is_none()).collect();
        if implicit.is_empty() {
            return Ok(None);
        }
        let Ok(matches) =
            self.unrequired(relax(cmd.clone(), prompted_args)).try_get_matches_from(argv)
        else {
            return Ok(None);
        };
        // Built, so a flag's action has filled in its implicit default.
        let mut built = cmd.clone();
        built.build();
        let Some(stdin_arg) = implicit.into_iter().find(|stdin_arg| {
            let (path, id) = (stdin_arg.subcommand_path.as_slice(), stdin_arg.attr.arg_id);
            unfilled(&built, &matches, path, id).is_some() && !blocked(cmd, &matches, path, id, &[])
        }) else {
            return Ok(None);
        };
        let Some(piped) =
            stdin.read(ReadWhen::Piped).map_err(ParseError::Io)?.filter(|piped| !piped.is_empty())
        else {
            return Ok(None);
        };
        StdinValue::decode(cmd, stdin_arg, piped).map(Some)
    }

    /// The arg that takes stdin, and its text: the one `dashed` reads, which takes all of stdin,
    /// else the one [`Self::piped`] fills.
    ///
    /// # Errors
    /// As [`Self::piped`].
    pub(super) fn read(
        &self,
        cmd: &Command,
        prompted_args: &[PromptedArg],
        dashed: &Dashed<'_>,
        argv: &[OsString],
        stdin: &impl Stdin,
    ) -> Result<Option<StdinValue<'_>>, ParseError> {
        let Some(stdin_arg) = self
            .iter()
            .find(|stdin_arg| dashed.reads(&stdin_arg.subcommand_path, stdin_arg.attr.arg_id))
        else {
            return self.piped(cmd, prompted_args, argv, stdin);
        };
        let all = stdin.read(ReadWhen::Always).map_err(ParseError::Io)?.unwrap_or_default();
        StdinValue::decode(cmd, stdin_arg, all).map(Some)
    }

    /// `cmd` with no stdin arg required, so one left out to take stdin does not stop a parse.
    pub(super) fn unrequired(&self, cmd: Command) -> Command {
        self.iter().fold(cmd, |cmd, stdin_arg| {
            unrequire(cmd, &stdin_arg.subcommand_path, stdin_arg.attr.arg_id)
        })
    }
}

impl<'stdin_arg> StdinValue<'stdin_arg> {
    /// `stdin`, read for `stdin_arg`, as its value.
    ///
    /// # Errors
    /// - `stdin` is longer than [`STDIN_LIMIT`].
    /// - `stdin` is not UTF-8.
    fn decode(
        cmd: &Command,
        stdin_arg: &'stdin_arg StdinArg,
        stdin: Vec<u8>,
    ) -> Result<Self, ParseError> {
        let refuse = |kind: ErrorKind, why: &str| {
            // Built, so the arg can be printed.
            let mut built = cmd.clone();
            built.build();
            let arg = arg_at(&built, &stdin_arg.subcommand_path, stdin_arg.attr.arg_id)
                .map_or_else(|| stdin_arg.attr.arg_id.to_owned(), ToString::to_string);
            let message = format!("{arg} got {why} from stdin");
            ParseError::Clap(clap::Error::raw(kind, message).format(&mut cmd.clone()))
        };
        if stdin.len() > STDIN_LIMIT {
            let mebibytes = STDIN_LIMIT >> 20u32;
            return Err(refuse(ErrorKind::ValueValidation, &format!("more than {mebibytes} MiB")));
        }
        let Ok(mut text) = String::from_utf8(stdin) else {
            return Err(refuse(ErrorKind::InvalidUtf8, "invalid UTF-8"));
        };
        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }
        Ok(Self { stdin_arg, text })
    }
}
