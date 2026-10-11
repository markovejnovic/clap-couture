//! `#[couture(stdin)]`: an arg the user left out takes what is piped into stdin, and
//! `stdin(dash)` one passed `-` takes all of stdin.

use std::ffi::OsString;

use clap::{Command, error::ErrorKind};

use super::{
    Failure,
    dash::Dashed,
    prompting::{arg_at, blocked, relax, unfilled, unrequire},
};
use crate::{
    interactive::{PromptedArg, StdinArg},
    os::stdin::{ReadWhen, Stdin},
};

/// The arg in `stdin_args` that takes stdin, and its text: the one `dashed` reads, else the one
/// [`piped`] fills. A `-` that asks for an arg leaves it to be asked, so no pipe fills that one.
///
/// # Errors
/// As [`piped`].
pub(super) fn read<'stdin_arg>(
    cmd: &Command,
    prompted_args: &[PromptedArg],
    stdin_args: &'stdin_arg [StdinArg],
    dashed: &Dashed,
    argv: &[OsString],
    stdin: &impl Stdin,
) -> Result<Option<(&'stdin_arg StdinArg, String)>, Failure> {
    if let Some(stdin_arg) = stdin_args
        .iter()
        .find(|stdin_arg| dashed.reads(&stdin_arg.subcommand_path, stdin_arg.attr.arg_id))
    {
        let text = stdin.read(ReadWhen::Always).map_err(Failure::Io)?.unwrap_or_default();
        return decode(cmd, stdin_arg, text).map(|text| Some((stdin_arg, text)));
    }
    let implicit: Vec<&StdinArg> = stdin_args
        .iter()
        .filter(|stdin_arg| {
            !stdin_arg.attr.dash && !dashed.asks(&stdin_arg.subcommand_path, stdin_arg.attr.arg_id)
        })
        .collect();
    piped(cmd, prompted_args, &implicit, argv, stdin)
}

/// The stdin arg on the path the user took, if it got no value, and the text piped in for it.
/// `None` when nothing is piped, or the arg conflicts with one the user passed. Stdin is read
/// only once the rest of `argv` parses, so `--help` never waits on it.
///
/// One line ending is dropped from the end, so `echo eu |` reads as `eu`.
///
/// # Errors
/// - Reading stdin failed.
/// - What was piped is not UTF-8.
fn piped<'stdin_arg>(
    cmd: &Command,
    prompted_args: &[PromptedArg],
    stdin_args: &[&'stdin_arg StdinArg],
    argv: &[OsString],
    stdin: &impl Stdin,
) -> Result<Option<(&'stdin_arg StdinArg, String)>, Failure> {
    if stdin_args.is_empty() {
        return Ok(None);
    }
    let relaxed = stdin_args.iter().fold(relax(cmd.clone(), prompted_args), |cmd, stdin_arg| {
        unrequire(cmd, &stdin_arg.subcommand_path, stdin_arg.attr.arg_id)
    });
    let Ok(matches) = relaxed.try_get_matches_from(argv) else { return Ok(None) };
    // Built, so a flag's action has filled in its implicit default.
    let mut built = cmd.clone();
    built.build();
    let Some(stdin_arg) = stdin_args.iter().copied().find(|stdin_arg| {
        let (path, id) = (stdin_arg.subcommand_path.as_slice(), stdin_arg.attr.arg_id);
        unfilled(&built, &matches, path, id).is_some() && !blocked(cmd, &matches, path, id, &[])
    }) else {
        return Ok(None);
    };
    let Some(piped) =
        stdin.read(ReadWhen::Piped).map_err(Failure::Io)?.filter(|piped| !piped.is_empty())
    else {
        return Ok(None);
    };
    decode(cmd, stdin_arg, piped).map(|text| Some((stdin_arg, text)))
}

/// `stdin` as `stdin_arg`'s text, without one trailing line ending.
///
/// # Errors
/// `stdin` is not UTF-8.
fn decode(cmd: &Command, stdin_arg: &StdinArg, stdin: Vec<u8>) -> Result<String, Failure> {
    let Ok(mut text) = String::from_utf8(stdin) else {
        // Built, so the arg can be printed.
        let mut built = cmd.clone();
        built.build();
        let arg = arg_at(&built, &stdin_arg.subcommand_path, stdin_arg.attr.arg_id)
            .map_or_else(|| stdin_arg.attr.arg_id.to_owned(), ToString::to_string);
        let message = format!("{arg} got invalid UTF-8 from stdin");
        return Err(Failure::Clap(
            clap::Error::raw(ErrorKind::InvalidUtf8, message).format(&mut cmd.clone()),
        ));
    };
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    Ok(text)
}
