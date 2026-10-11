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
    /// Exits with status 130 when the user cancels a prompt, and 1 when they decline a
    /// confirmation.
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
    Cancelled,
    /// clap rejected the arguments, or a prompt failed.
    Clap(clap::Error),
    /// The user answered no to a confirmation.
    #[cfg(feature = "interactive")]
    Declined,
}

impl Failure {
    /// Exit the process the way clap would for this failure; 130 for a cancelled prompt, 1 for a
    /// declined confirmation.
    fn exit(self) -> ! {
        match self {
            // 130 is the conventional status for an interrupted command.
            #[cfg(feature = "interactive")]
            Self::Cancelled => std::process::exit(130),
            Self::Clap(err) => err.exit(),
            #[cfg(feature = "interactive")]
            Self::Declined => std::process::exit(1),
        }
    }

    fn into_clap(self) -> clap::Error {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled => clap::Error::raw(clap::error::ErrorKind::Io, "prompt cancelled\n"),
            Self::Clap(err) => err,
            #[cfg(feature = "interactive")]
            Self::Declined => {
                clap::Error::raw(clap::error::ErrorKind::Io, "confirmation declined\n")
            }
        }
    }
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
