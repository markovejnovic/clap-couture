//! Parse entry points with couture installed.

#[cfg(feature = "interactive")]
mod prompting;
#[cfg(feature = "interactive")]
mod secrets;

use std::ffi::OsString;

use clap::{Command, FromArgMatches, Parser};

#[cfg(feature = "interactive")]
use crate::interactive::Backend;
#[cfg(feature = "interactive")]
use crate::os::signal::Signal;
use crate::{CommandExt as _, Couture};

/// clap's [`Parser`] entry points, with couture's help installed.
///
/// Implemented for every [`Parser`] that also implements [`Couture`]. With an `interactive-*`
/// backend feature on, every entry point asks for prompted args the user left out.
pub trait CoutureParser: Parser + Couture {
    /// The clap [`Command`] with couture's help installed, and its secret args guarded.
    #[must_use]
    fn couture_command() -> Command {
        let cmd = Self::command().with_couture::<Self>();
        #[cfg(feature = "interactive")]
        let cmd = guard_secrets::<Self>(cmd);
        cmd
    }

    /// Equivalent to [`Parser::parse`], with couture's help.
    ///
    /// When the user cancels a prompt, dies of Ctrl-C's signal, or of the one the editor died of,
    /// so a shell loop around the CLI stops too; exits with status 130 where there are no
    /// signals. Exits with status 1 when a prompt fails or a confirmation is declined.
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
    /// [`Io`](clap::error::ErrorKind::Io) when a prompt fails, is cancelled or is declined.
    fn couture_try_parse() -> Result<Self, clap::Error> {
        Self::couture_try_parse_from(std::env::args_os())
    }

    /// Equivalent to [`Parser::try_parse_from`], with couture's help.
    ///
    /// # Errors
    /// The [`clap::Error`] clap would report for the same arguments, or one of kind
    /// [`Io`](clap::error::ErrorKind::Io) when a prompt fails, is cancelled or is declined.
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
    Cancelled(Signal),
    /// clap rejected the arguments.
    Clap(clap::Error),
    /// The user answered no to a confirmation.
    #[cfg(feature = "interactive")]
    Declined,
    /// A prompt failed: the terminal, the temp file or the editor.
    #[cfg(feature = "interactive")]
    Io(std::io::Error),
}

impl Failure {
    /// Exit the process the way clap would for this failure; die of the signal for a cancelled
    /// prompt, and exit 1 for a failed one or a declined confirmation, which is no usage error.
    fn exit(self) -> ! {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled(signal) => signal.die(),
            Self::Clap(err) => err.exit(),
            #[cfg(feature = "interactive")]
            Self::Declined => std::process::exit(1),
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
            Self::Cancelled(_) => {
                clap::Error::raw(clap::error::ErrorKind::Io, "prompt cancelled\n")
            }
            Self::Clap(err) => err,
            #[cfg(feature = "interactive")]
            Self::Declined => {
                clap::Error::raw(clap::error::ErrorKind::Io, "confirmation declined\n")
            }
            #[cfg(feature = "interactive")]
            Self::Io(err) => io_error(&err),
        }
    }
}

/// `err` as a clap error of kind [`Io`](clap::error::ErrorKind::Io), with clap's `error:` styling.
#[cfg(feature = "interactive")]
fn io_error(err: &std::io::Error) -> clap::Error {
    clap::Error::raw(clap::error::ErrorKind::Io, format!("{err}\n"))
}
/// Guard `T`'s secret args, so every parse of the command refuses them on the command line.
#[cfg(feature = "interactive")]
fn guard_secrets<T>(cmd: Command) -> Command
where
    T: Couture,
{
    let tree: &'static crate::interactive::PromptedArgTree = &T::PROMPTED_ARGS;
    secrets::guard(cmd, &tree.prompted_args(), &secrets::Vault::default())
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
    #[cfg(all(unix, feature = "interactive"))]
    use std::{os::unix::process::ExitStatusExt as _, process::Command};

    #[cfg(all(unix, feature = "interactive"))]
    use rstest::rstest;

    #[cfg(all(unix, feature = "interactive"))]
    use super::Failure;
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
        Failure::Cancelled(signal).exit();
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
