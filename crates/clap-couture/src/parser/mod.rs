//! Parse entry points with couture installed.

#[cfg(feature = "interactive")]
mod prompting;

use std::ffi::OsString;

use clap::{Command, FromArgMatches, Parser};

#[cfg(feature = "interactive")]
use crate::interactive::Backend;
use crate::{CommandExt as _, Couture};

/// clap's [`Parser`] entry points, with couture's help installed.
///
/// Implemented for every [`Parser`] that also implements [`Couture`]. With an `interactive-*`
/// backend feature on, every entry point asks for prompted args the user left out.
pub trait CoutureParser: Parser + Couture {
    /// The clap [`Command`] with couture's help installed.
    #[must_use]
    fn couture_command() -> Command {
        Self::command().with_couture::<Self>()
    }

    /// Equivalent to [`Parser::parse`], with couture's help.
    ///
    /// Exits with status 130 when the user cancels a prompt.
    #[must_use]
    fn couture_parse() -> Self {
        Self::couture_parse_from(std::env::args_os())
    }

    /// Equivalent to [`Parser::parse_from`], with couture's help.
    #[must_use]
    fn couture_parse_from<I, T>(itr: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        parse_default(&itr.into_iter().map(Into::into).collect::<Vec<OsString>>())
            .unwrap_or_else(|failure| failure.exit())
    }

    /// Equivalent to [`Parser::try_parse`], with couture's help.
    ///
    /// # Errors
    /// The [`clap::Error`] clap would report for the same arguments, or one of kind
    /// [`Io`](clap::error::ErrorKind::Io) when a prompt fails or is cancelled.
    fn couture_try_parse() -> Result<Self, clap::Error> {
        Self::couture_try_parse_from(std::env::args_os())
    }

    /// Equivalent to [`Parser::try_parse_from`], with couture's help.
    ///
    /// # Errors
    /// The [`clap::Error`] clap would report for the same arguments, or one of kind
    /// [`Io`](clap::error::ErrorKind::Io) when a prompt fails or is cancelled.
    fn couture_try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        parse_default(&itr.into_iter().map(Into::into).collect::<Vec<OsString>>())
            .map_err(Failure::into_clap)
    }

    /// Equivalent to [`couture_try_parse_from`](Self::couture_try_parse_from), asking `backend`
    /// for prompted args.
    ///
    /// # Errors
    /// As [`couture_try_parse_from`](Self::couture_try_parse_from).
    #[cfg(feature = "interactive")]
    fn couture_try_parse_from_with<I, T>(backend: &dyn Backend, itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        prompting::parse(
            Self::couture_command(),
            &Self::PROMPTED_ARGS,
            &Self::MESSAGES,
            &itr.into_iter().map(Into::into).collect::<Vec<OsString>>(),
            backend,
        )
        .map_err(Failure::into_clap)
    }
}

impl<T> CoutureParser for T where T: Parser + Couture {}

/// Why a parse produced no value.
pub(crate) enum Failure {
    /// The user backed out of a prompt.
    #[cfg(feature = "interactive")]
    Cancelled {
        /// The signal the editor died of, if it did; backing out otherwise counts as Ctrl-C.
        signal: Option<i32>,
    },
    /// clap rejected the arguments.
    Clap(clap::Error),
    /// A prompt failed: the terminal, the temp file or the editor.
    #[cfg(feature = "interactive")]
    Io(std::io::Error),
}

impl Failure {
    /// Exit the process the way clap would for this failure; die of the signal for a cancelled
    /// prompt, and exit 1 for a failed one, which is no usage error.
    fn exit(self) -> ! {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled { signal } => die_of(signal),
            Self::Clap(err) => err.exit(),
            #[cfg(feature = "interactive")]
            Self::Io(err) => {
                // Nothing is left to report a failed print to.
                drop(io_error(&err).print());
                std::process::exit(1)
            }
        }
    }

    fn into_clap(self) -> clap::Error {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled { signal: _ } => {
                clap::Error::raw(clap::error::ErrorKind::Io, "prompt cancelled\n")
            }
            Self::Clap(err) => err,
            #[cfg(feature = "interactive")]
            Self::Io(err) => io_error(&err),
        }
    }
}

/// Die of `signal`, SIGINT when `None`, as git does: a shell only stops a loop or script around a
/// command that died of Ctrl-C, and takes any exit status, 130 included, as the command having
/// handled it. Exits with the shell's code for the signal where there are no signals to die of,
/// or if the signal somehow doesn't kill.
#[cfg(feature = "interactive")]
#[expect(clippy::exit, reason = "the fallback when there is no signal to die of")]
fn die_of(signal: Option<i32>) -> ! {
    #[cfg(unix)]
    let signal = signal.unwrap_or(libc::SIGINT);
    #[cfg(unix)]
    {
        use std::io::Write as _;

        // Dying skips Rust's own flushing; nothing is left to report a failed flush to.
        drop(std::io::stdout().flush());
        drop(std::io::stderr().flush());
        // SAFETY: an all-zero `sigset_t` is valid storage, filled in by `sigemptyset`.
        let mut set: libc::sigset_t = unsafe { core::mem::zeroed() };
        // A blocked signal would stay pending instead of killing, so it is unblocked first.
        // SAFETY: `set` is live for the call.
        unsafe { libc::sigemptyset(&raw mut set) };
        // SAFETY: as above; `signal` is a real signal, the one that ended the prompt.
        unsafe { libc::sigaddset(&raw mut set, signal) };
        // SAFETY: `set` is live, and the old mask is not asked for.
        unsafe { libc::pthread_sigmask(libc::SIG_UNBLOCK, &raw const set, core::ptr::null_mut()) };
        // SAFETY: the default action is valid for every signal.
        unsafe { libc::signal(signal, libc::SIG_DFL) };
        // SAFETY: raising a real signal; with its default action, it ends the process.
        unsafe { libc::raise(signal) };
    }
    // 128 plus the signal is how a shell reports a death by it: 130 for SIGINT.
    #[cfg(not(unix))]
    let signal = signal.unwrap_or(2i32);
    std::process::exit(128i32.saturating_add(signal))
}

/// `err` as a clap error of kind [`Io`](clap::error::ErrorKind::Io), with clap's `error:` styling.
#[cfg(feature = "interactive")]
fn io_error(err: &std::io::Error) -> clap::Error {
    clap::Error::raw(clap::error::ErrorKind::Io, format!("{err}\n"))
}

/// Parse through [`DefaultBackend`](crate::interactive::DefaultBackend), or without prompting
/// when the `interactive` feature is off.
fn parse_default<T>(argv: &[OsString]) -> Result<T, Failure>
where
    T: CoutureParser,
{
    #[cfg(feature = "interactive")]
    {
        let backend = crate::interactive::DefaultBackend::default();
        prompting::parse(T::couture_command(), &T::PROMPTED_ARGS, &T::MESSAGES, argv, &backend)
    }
    #[cfg(not(feature = "interactive"))]
    {
        parse(T::couture_command(), argv)
    }
}

/// Parse `argv` against `cmd` as clap would, then build `T`. The prompting parse ends here too.
fn parse<T>(mut cmd: Command, argv: &[OsString]) -> Result<T, Failure>
where
    T: FromArgMatches,
{
    let mut matches = cmd.try_get_matches_from_mut(argv).map_err(Failure::Clap)?;
    T::from_arg_matches_mut(&mut matches).map_err(|err| Failure::Clap(err.format(&mut cmd)))
}

#[cfg(test)]
mod tests {
    #![cfg(all(unix, feature = "interactive"))]

    use std::{os::unix::process::ExitStatusExt as _, process::Command};

    use rstest::rstest;

    use super::Failure;

    /// Set to the signal for [`die_in_child`] to die of, or empty for none.
    const DIE_OF: &str = "CLAP_COUTURE_TEST_DIE_OF";

    /// Run by [`a_cancelled_parse_dies_of_its_signal`] in a child process of this binary.
    #[test]
    #[ignore = "run in a child process by `a_cancelled_parse_dies_of_its_signal`"]
    fn die_in_child() {
        let Ok(signal) = std::env::var(DIE_OF) else { return };
        Failure::Cancelled { signal: signal.parse().ok() }.exit();
    }

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
