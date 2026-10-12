//! Parse entry points with couture installed.

#[cfg(all(test, feature = "interactive"))]
mod clap_parity;
#[cfg(feature = "interactive")]
pub(crate) mod dash;
#[cfg(feature = "interactive")]
mod prompting;
#[cfg(feature = "interactive")]
mod secrets;
#[cfg(feature = "interactive")]
mod stdin;

use core::fmt;
use std::ffi::OsString;

use clap::{Command, CommandFactory, FromArgMatches, Parser};

#[cfg(feature = "interactive")]
use crate::interactive::{Backend, DefaultBackend, Prompter};
#[cfg(feature = "interactive")]
use crate::os::signal::Signal;
use crate::{CommandExt as _, Couture, help_tree::DeferredHelp};

/// clap's [`Parser`] entry points, with couture's help installed.
///
/// Implemented for every [`Parser`] that also implements [`Couture`]. With an `interactive-*`
/// backend feature on, every entry point asks for prompted args the user left out, through
/// [`DefaultBackend`] or, for the `_with` ones, the backend given.
pub trait CoutureParser: Parser + Couture {
    /// The clap [`Command`] with couture's help installed, and its secret args guarded.
    #[must_use]
    fn couture_command() -> Command {
        guarded_command::<Self>().with_couture::<Self>()
    }

    /// Equivalent to [`Parser::parse`], with couture's help.
    ///
    /// Exits as [`ParseError::exit`] does on a failed parse.
    #[must_use]
    fn couture_parse() -> Self {
        Self::couture_parse_from(std::env::args_os())
    }

    /// Equivalent to [`Parser::parse_from`], with couture's help.
    ///
    /// Exits as [`ParseError::exit`] does on a failed parse.
    #[must_use]
    fn couture_parse_from<I, T>(itr: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::couture_try_parse_from(itr).unwrap_or_else(|err| err.exit())
    }

    /// Equivalent to [`couture_parse_from`](Self::couture_parse_from), asking `backend` for
    /// prompted args.
    #[cfg(feature = "interactive")]
    #[must_use]
    fn couture_parse_from_with<B, I, T>(backend: B, itr: I) -> Self
    where
        B: Backend,
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::couture_try_parse_from_with(backend, itr).unwrap_or_else(|err| err.exit())
    }

    /// Equivalent to [`couture_parse`](Self::couture_parse), asking `backend` for prompted args.
    #[cfg(feature = "interactive")]
    #[must_use]
    fn couture_parse_with<B>(backend: B) -> Self
    where
        B: Backend,
    {
        Self::couture_parse_from_with(backend, std::env::args_os())
    }

    /// Equivalent to [`Parser::try_parse`], with couture's help.
    ///
    /// # Errors
    /// See [`ParseError`].
    fn couture_try_parse() -> Result<Self, ParseError> {
        Self::couture_try_parse_from(std::env::args_os())
    }

    /// Equivalent to [`Parser::try_parse_from`], with couture's help.
    ///
    /// # Errors
    /// See [`ParseError`].
    fn couture_try_parse_from<I, T>(itr: I) -> Result<Self, ParseError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        #[cfg(feature = "interactive")]
        {
            Self::couture_try_parse_from_with(DefaultBackend::default(), itr)
        }
        #[cfg(not(feature = "interactive"))]
        {
            parse(
                guarded_command::<Self>(),
                &itr.into_iter().map(Into::into).collect::<Vec<_>>(),
                &DeferredHelp::of::<Self>(guarded_command::<Self>),
            )
        }
    }

    /// Equivalent to [`couture_try_parse_from`](Self::couture_try_parse_from), asking `backend`
    /// for prompted args.
    ///
    /// # Errors
    /// See [`ParseError`].
    #[cfg(feature = "interactive")]
    fn couture_try_parse_from_with<B, I, T>(backend: B, itr: I) -> Result<Self, ParseError>
    where
        B: Backend,
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        prompting::parse(
            guarded_command::<Self>(),
            &Self::PROMPTED_ARGS,
            &Self::MESSAGES,
            &itr.into_iter().map(Into::into).collect::<Vec<OsString>>(),
            &backend,
            &crate::os::stdin::ProcessStdin,
            &DeferredHelp::of::<Self>(guarded_command::<Self>),
        )
    }

    /// Equivalent to [`couture_try_parse`](Self::couture_try_parse), asking `backend` for
    /// prompted args.
    ///
    /// # Errors
    /// See [`ParseError`].
    #[cfg(feature = "interactive")]
    fn couture_try_parse_with<B>(backend: B) -> Result<Self, ParseError>
    where
        B: Backend,
    {
        Self::couture_try_parse_from_with(backend, std::env::args_os())
    }

    /// A [`Prompter`] asking through [`DefaultBackend`], in this command's styles and
    /// [`Messages`](crate::Messages); `None` without a terminal.
    #[cfg(feature = "interactive")]
    #[must_use]
    fn prompter() -> Option<Prompter> {
        Self::prompter_with(DefaultBackend::default())
    }

    /// Equivalent to [`prompter`](Self::prompter), but sure there is a terminal.
    ///
    /// # Panics
    /// Without a terminal to ask on.
    #[cfg(feature = "interactive")]
    #[must_use]
    #[track_caller]
    #[expect(clippy::expect_used, reason = "panicking without a terminal is its contract")]
    fn prompter_unwrap() -> Prompter {
        Self::prompter().expect("`prompter_unwrap` needs a terminal to ask on")
    }

    /// Equivalent to [`prompter`](Self::prompter), asking through `backend`.
    #[cfg(feature = "interactive")]
    #[must_use]
    fn prompter_with<B>(backend: B) -> Option<Prompter<B>>
    where
        B: Backend,
    {
        Prompter::new(backend, Self::MESSAGES, &Self::command())
    }
}

impl<T> CoutureParser for T where T: Parser + Couture {}

/// Why a [`CoutureParser`] parse produced no value.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// The user backed out of a prompt, which counts as Ctrl-C, or the editor died of this
    /// signal.
    #[cfg(feature = "interactive")]
    Cancelled(Signal),
    /// clap rejected the arguments, or the user asked for `--help` or `--version`.
    Clap(clap::Error),
    /// The user answered no to a `#[couture(confirm)]`.
    #[cfg(feature = "interactive")]
    Declined,
    /// A prompt failed: the terminal, the temp file, the editor, or reading stdin.
    #[cfg(feature = "interactive")]
    Io(std::io::Error),
}

impl ParseError {
    /// Exit the process as [`couture_parse`](CoutureParser::couture_parse) does.
    ///
    /// clap's errors exit as [`clap::Error::exit`] does: `--help` with status 0, a usage error
    /// with 2. A cancelled prompt dies of its signal, so a shell loop around the CLI stops too,
    /// or exits with status 130 where there are no signals. An app that handles or ignores the
    /// signal gets it under its own handling instead and, if it survives, exits with 128 plus the
    /// signal: 130 for Ctrl-C. A failed prompt prints why and exits with status 1, as does a
    /// declined confirmation, silently.
    pub fn exit(self) -> ! {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled(signal) => signal.die(),
            Self::Clap(err) => err.exit(),
            #[cfg(feature = "interactive")]
            Self::Declined => std::process::exit(1),
            #[cfg(feature = "interactive")]
            Self::Io(err) => {
                // clap's `error:` styling; nothing is left to report a failed print to.
                let err = clap::Error::raw(clap::error::ErrorKind::Io, format!("{err}\n"));
                drop(err.print());
                std::process::exit(1)
            }
        }
    }

    /// The kind of clap's error; `None` when a prompt, not clap, ended the parse.
    #[must_use]
    pub fn kind(&self) -> Option<clap::error::ErrorKind> {
        match self {
            Self::Clap(err) => Some(err.kind()),
            #[cfg(feature = "interactive")]
            Self::Cancelled(_) | Self::Declined | Self::Io(_) => None,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled(_) => f.write_str("prompt cancelled"),
            Self::Clap(err) => err.fmt(f),
            #[cfg(feature = "interactive")]
            Self::Declined => f.write_str("confirmation declined"),
            #[cfg(feature = "interactive")]
            Self::Io(err) => err.fmt(f),
        }
    }
}

impl core::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Clap(err) => Some(err),
            #[cfg(feature = "interactive")]
            Self::Io(err) => Some(err),
            #[cfg(feature = "interactive")]
            Self::Cancelled(_) | Self::Declined => None,
        }
    }
}

impl From<clap::Error> for ParseError {
    fn from(err: clap::Error) -> Self {
        Self::Clap(err)
    }
}

/// Guard `T`'s secret args, so every parse of the command refuses them on the command line.
#[cfg(feature = "interactive")]
fn guard_secrets<T>(cmd: Command) -> Command
where
    T: Couture,
{
    let prompted_arg_tree: &'static crate::interactive::PromptedArgTree = &T::PROMPTED_ARGS;
    secrets::guard(
        cmd,
        &prompted_arg_tree.prompted_args(),
        &secrets::Vault::default(),
        &T::MESSAGES.environment.get(),
    )
}

/// `T`'s command, its secret args guarded, without couture's help.
fn guarded_command<T>() -> Command
where
    T: CommandFactory + Couture,
{
    let cmd = T::command();
    // Guarded first, so the help couture renders hides the refused secrets.
    #[cfg(feature = "interactive")]
    let cmd = guard_secrets::<T>(cmd);
    cmd
}

/// Parse `argv` against `cmd` as clap would, printing `help`, then build `T`. The prompting parse
/// ends here too.
fn parse<T>(mut cmd: Command, argv: &[OsString], help: &DeferredHelp) -> Result<T, ParseError>
where
    T: FromArgMatches,
{
    let mut matches = help.matches(&mut cmd, argv)?;
    T::from_arg_matches_mut(&mut matches).map_err(|err| ParseError::Clap(err.format(&mut cmd)))
}

#[cfg(test)]
mod tests {
    #[cfg(all(unix, feature = "interactive"))]
    use std::{os::unix::process::ExitStatusExt as _, process::Command};

    #[cfg(all(unix, feature = "interactive"))]
    use rstest::rstest;

    #[cfg(all(unix, feature = "interactive"))]
    use super::ParseError;
    #[cfg(all(unix, feature = "interactive"))]
    use crate::os::signal::Signal;

    #[cfg(all(unix, feature = "interactive"))]
    /// Set to the signal for [`die_in_child`] to die of, or empty for none.
    const DIE_OF: &str = "CLAP_COUTURE_TEST_DIE_OF";

    #[cfg(all(unix, feature = "interactive"))]
    /// Run by [`a_cancelled_parse_dies_of_its_signal`] in a child process of this binary.
    #[test]
    #[ignore = "run in a child process by `a_cancelled_parse_dies_of_its_signal`"]
    fn die_in_child() {
        let Ok(signal) = std::env::var(DIE_OF) else { return };
        let signal = signal.parse().ok().and_then(Signal::from_raw).unwrap_or(Signal::Interrupt);
        // A test ignoring the signal in the runner meanwhile would hand that down.
        // SAFETY: the default action is valid for every signal.
        unsafe {
            libc::signal(signal.to_raw(), libc::SIG_DFL);
        }
        ParseError::Cancelled(signal).exit();
    }

    #[cfg(all(unix, feature = "interactive"))]
    #[rstest]
    #[case::backed_out("", libc::SIGINT)]
    #[case::editor_hung_up("1", libc::SIGHUP)]
    #[case::editor_interrupted("2", libc::SIGINT)]
    fn a_cancelled_parse_dies_of_its_signal(#[case] editor_signal: &str, #[case] died_of: i32) {
        let status = std::env::current_exe().and_then(|test_binary| {
            Command::new(test_binary)
                .args(["parser::tests::die_in_child", "--exact", "--ignored", "--quiet"])
                .env(DIE_OF, editor_signal)
                .output()
        });
        let signal = status.ok().and_then(|output| output.status.signal());
        assert_eq!(signal, Some(died_of));
    }
}
