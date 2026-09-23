//! Parse entry points with couture installed.

use std::ffi::OsString;

use clap::{Command, Parser};

#[cfg(feature = "interactive")]
use crate::interactive::Prompter;
use crate::{CommandExt as _, Couture};

/// clap's [`Parser`] entry points, with couture's help installed.
///
/// Implemented for every [`Parser`] that also implements [`Couture`]. With an `interactive-*`
/// backend feature on, every entry point asks for marked args the user left out; see
/// [`interactive`](crate::interactive).
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
    ///
    /// Exits with status 130 when the user cancels a prompt.
    #[must_use]
    fn couture_parse_from<I, T>(itr: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        exit_on_failure(parse_default(&collect(itr)))
    }

    /// Equivalent to [`couture_parse`](Self::couture_parse), asking `prompter` for marked args.
    #[cfg(feature = "interactive")]
    #[must_use]
    fn couture_parse_with(prompter: &dyn Prompter) -> Self {
        let argv = collect(std::env::args_os());
        exit_on_failure(crate::interactive::run(
            Self::couture_command(),
            &Self::PROMPTS,
            &argv,
            prompter,
        ))
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
        parse_default(&collect(itr)).map_err(Failure::into_clap)
    }

    /// Equivalent to [`couture_try_parse_from`](Self::couture_try_parse_from), asking `prompter`
    /// for marked args.
    ///
    /// # Errors
    /// As [`couture_try_parse_from`](Self::couture_try_parse_from).
    #[cfg(feature = "interactive")]
    fn couture_try_parse_from_with<I, T>(
        prompter: &dyn Prompter,
        itr: I,
    ) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        crate::interactive::run(Self::couture_command(), &Self::PROMPTS, &collect(itr), prompter)
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
}

impl Failure {
    fn into_clap(self) -> clap::Error {
        match self {
            #[cfg(feature = "interactive")]
            Self::Cancelled => clap::Error::raw(clap::error::ErrorKind::Io, "prompt cancelled\n"),
            Self::Clap(err) => err,
        }
    }
}

fn collect<I, T>(itr: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    itr.into_iter().map(Into::into).collect()
}

fn exit_on_failure<T>(result: Result<T, Failure>) -> T {
    match result {
        Ok(value) => value,
        #[cfg(feature = "interactive")]
        Err(Failure::Cancelled) => exit_cancelled(),
        Err(Failure::Clap(err)) => err.exit(),
    }
}

#[cfg(feature = "interactive")]
#[expect(clippy::exit, reason = "130 is the conventional status for an interrupted command")]
fn exit_cancelled() -> ! {
    std::process::exit(130)
}

/// Parse through [`DefaultPrompter`](crate::interactive::DefaultPrompter).
#[cfg(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))]
fn parse_default<T>(argv: &[OsString]) -> Result<T, Failure>
where
    T: CoutureParser,
{
    let prompter = crate::interactive::DefaultPrompter::default();
    crate::interactive::run(T::couture_command(), &T::PROMPTS, argv, &prompter)
}

/// Parse without prompting: no backend is enabled.
#[cfg(not(any(feature = "interactive-cliclack", feature = "interactive-dialoguer")))]
fn parse_default<T>(argv: &[OsString]) -> Result<T, Failure>
where
    T: CoutureParser,
{
    let mut cmd = T::couture_command();
    let mut matches = cmd.try_get_matches_from_mut(argv).map_err(Failure::Clap)?;
    T::from_arg_matches_mut(&mut matches).map_err(|err| Failure::Clap(err.format(&mut cmd)))
}
