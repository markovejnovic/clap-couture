//! `#[couture(accept_stdin)]`: an arg the user left out takes what is piped into stdin.

use std::{ffi::OsString, io};

use clap::{Command, error::ErrorKind};

use super::{
    Failure,
    prompting::{arg_at, blocked, relax, unfilled, unrequire},
};
use crate::interactive::{PromptedArg, StdinArg};

/// The stdin arg on the path the user took, if it got no value, and the text piped in for it.
/// `None` when nothing is piped, or the arg conflicts with one the user passed. Stdin is read
/// only once the rest of `argv` parses, so `--help` never waits on it.
///
/// One line ending is dropped from the end, so `echo eu |` reads as `eu`.
///
/// # Errors
/// - Reading stdin failed.
/// - What was piped is not UTF-8.
pub(super) fn piped<'stdin_arg>(
    cmd: &Command,
    prompted_args: &[PromptedArg],
    stdin_args: &'stdin_arg [StdinArg],
    argv: &[OsString],
    read_piped_stdin: impl FnOnce() -> io::Result<Option<Vec<u8>>>,
) -> Result<Option<(&'stdin_arg StdinArg, String)>, Failure> {
    if stdin_args.is_empty() {
        return Ok(None);
    }
    let relaxed = stdin_args.iter().fold(relax(cmd.clone(), prompted_args), |cmd, stdin_arg| {
        unrequire(cmd, &stdin_arg.subcommand_path, stdin_arg.arg_id)
    });
    let Ok(matches) = relaxed.try_get_matches_from(argv) else { return Ok(None) };
    // Built, so a flag's action has filled in its implicit default, and an arg can be printed.
    let mut built = cmd.clone();
    built.build();
    let Some(stdin_arg) = stdin_args.iter().find(|stdin_arg| {
        let (path, id) = (stdin_arg.subcommand_path.as_slice(), stdin_arg.arg_id);
        unfilled(&built, &matches, path, id).is_some() && !blocked(cmd, &matches, path, id, &[])
    }) else {
        return Ok(None);
    };
    let Some(piped) = read_piped_stdin().map_err(Failure::Io)?.filter(|piped| !piped.is_empty())
    else {
        return Ok(None);
    };
    let Ok(mut text) = String::from_utf8(piped) else {
        let arg = arg_at(&built, &stdin_arg.subcommand_path, stdin_arg.arg_id)
            .map_or_else(|| stdin_arg.arg_id.to_owned(), ToString::to_string);
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
    Ok(Some((stdin_arg, text)))
}
