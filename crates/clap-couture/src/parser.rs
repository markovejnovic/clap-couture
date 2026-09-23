//! Parse entry points with couture installed.

use std::ffi::OsString;

use clap::{Command, Parser};

use crate::{CommandExt as _, Couture};

/// clap's [`Parser`] entry points, with couture's help installed.
///
/// Implemented for every [`Parser`] that also implements [`Couture`].
pub trait CoutureParser: Parser + Couture {
    /// The clap [`Command`] with couture's help installed.
    #[must_use]
    fn couture_command() -> Command {
        Self::command().with_couture::<Self>()
    }

    /// Equivalent to [`Parser::parse`], with couture's help.
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
        Self::couture_try_parse_from(itr).unwrap_or_else(|err| err.exit())
    }

    /// Equivalent to [`Parser::try_parse`], with couture's help.
    ///
    /// # Errors
    /// The [`clap::Error`] clap would report for the same arguments.
    fn couture_try_parse() -> Result<Self, clap::Error> {
        Self::couture_try_parse_from(std::env::args_os())
    }

    /// Equivalent to [`Parser::try_parse_from`], with couture's help.
    ///
    /// # Errors
    /// The [`clap::Error`] clap would report for the same arguments.
    fn couture_try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let mut cmd = Self::couture_command();
        let mut matches = cmd.try_get_matches_from_mut(itr)?;
        Self::from_arg_matches_mut(&mut matches).map_err(|err| err.format(&mut cmd))
    }
}

impl<T> CoutureParser for T where T: Parser + Couture {}
