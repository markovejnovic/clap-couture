//! Beautiful, categorized `--help` for [`clap`] CLIs.
//!
//! # Example
//!
//! ```
//! use clap::{Parser, Subcommand};
//! use clap_couture::{Couture, CoutureParser};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[derive(Subcommand, Couture)]
//! #[couture(categories = {
//!     "everyday" = { description = "what you'll reach for daily" },
//!     "admin" = {},
//! })]
//! enum Cmd {
//!     /// Search your deployment history
//!     #[category("everyday")]
//!     Search,
//!     /// Configure the tool
//!     #[category("admin")]
//!     Config,
//! }
//!
//! let cli = Cli::couture_parse_from(["app", "search"]);
//! assert!(matches!(cli.cmd, Cmd::Search));
//! ```
//!
//! A `#[category("...")]` that isn't declared in the enum's `#[couture(...)]`
//! fails to compile:
//!
//! ```compile_fail
//! use clap::Subcommand;
//! use clap_couture::Couture;
//!
//! #[derive(Subcommand, Couture)]
//! #[couture(categories = { "everyday" = {} })]
//! enum Cmd {
//!     #[category("typo")]
//!     Search,
//! }
//! ```
//!
//! The same holds for a category an enum only inherits from a parent command:
//!
//! ```compile_fail
//! use clap::Subcommand;
//! use clap_couture::Couture;
//!
//! #[derive(Subcommand, Couture)]
//! #[couture(inherit = ["sync"])]
//! enum Cmd {
//!     #[category("snyc")]
//!     Login,
//! }
//! ```
//!
//! The subcommands stay visible to clap, so shell completions and
//! `tool <cmd> --help` are unaffected -- only the help *listing* changes.

mod parser;
mod render;

use clap::{Command, builder::StyledStr};
pub use clap_couture_derive::Couture;
pub use parser::CoutureParser;
use render::CommandRenderExt as _;

/// A clap subcommand name, eg. `foo` in `app foo`.
pub struct CommandName(&'static str);

impl CommandName {
    /// The underlying subcommand name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }

    /// Wrap a static subcommand name.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }
}

/// Defines a category of similar commands.
pub struct Category {
    /// Text printed beside the heading, if any.
    pub description: Option<&'static str>,

    /// Key that `#[category("...")]` refers to.
    pub label: &'static str,

    /// Heading shown to the user; falls back to [`Self::label`].
    pub title: Option<&'static str>,
}

impl Category {
    /// The user-facing heading: [`Self::title`], falling back to [`Self::label`].
    pub(crate) const fn heading(&self) -> &'static str {
        match self.title {
            Some(title) => title,
            None => self.label,
        }
    }
}

/// A static map from subcommand name to the category it belongs to.
#[derive(Clone, Copy)]
pub struct CommandCategoryMap(&'static [(CommandName, Category)]);

impl CommandCategoryMap {
    /// A map with no entries.
    pub const EMPTY: Self = Self(&[]);

    /// The distinct categories, in first-appearance order.
    pub(crate) fn distinct(self) -> impl Iterator<Item = &'static Category> {
        let entries = self.0;
        entries
            .iter()
            .enumerate()
            .filter(move |(i, entry)| {
                entries.iter().position(|first| first.1.label == entry.1.label) == Some(*i)
            })
            .map(|(_, entry)| &entry.1)
    }

    /// The category assigned to `name`, if any.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Category> {
        self.0.iter().find(|entry| entry.0.as_str() == name).map(|entry| &entry.1)
    }

    /// Iterate the `(command, category)` pairs in declared order.
    pub fn iter(&self) -> core::slice::Iter<'_, (CommandName, Category)> {
        self.0.iter()
    }

    /// Wrap a static slice of `(command, category)` pairs.
    #[must_use]
    pub const fn new(entries: &'static [(CommandName, Category)]) -> Self {
        Self(entries)
    }
}

impl<'map> IntoIterator for &'map CommandCategoryMap {
    type IntoIter = core::slice::Iter<'map, (CommandName, Category)>;
    type Item = &'map (CommandName, Category);

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// A type's command categories, usually from `#[derive(Couture)]` on a subcommand enum.
pub trait Couture {
    /// Each command paired with the category it belongs to.
    const CATEGORIES: CommandCategoryMap = CommandCategoryMap::EMPTY;
}

/// Extension trait adding categorized help to a [`clap::Command`].
pub trait CommandExt {
    /// Group this command's `--help` subcommand listing by `T`'s categories.
    ///
    /// Overwrites the command's [`help_template`](Command::help_template) and
    /// [`before_help`](Command::before_help).
    ///
    /// # Example
    ///
    /// ```
    /// use clap::{CommandFactory, Parser, Subcommand};
    /// use clap_couture::{CommandExt, Couture};
    ///
    /// #[derive(Parser)]
    /// struct Cli {
    ///     #[command(subcommand)]
    ///     cmd: Cmd,
    /// }
    ///
    /// #[derive(Subcommand, Couture)]
    /// #[couture(categories = { "everyday" = { description = "daily drivers" } })]
    /// enum Cmd {
    ///     /// Search your deployment history
    ///     #[category("everyday")]
    ///     Search,
    /// }
    ///
    /// let mut cmd = Cli::command().with_couture::<Cmd>();
    /// assert!(cmd.render_help().to_string().contains("everyday"));
    /// ```
    #[must_use]
    fn with_couture<T>(self) -> Self
    where
        T: Couture;
}

impl CommandExt for Command {
    fn with_couture<T>(self) -> Self
    where
        T: Couture,
    {
        let styles = self.get_styles();
        let mut before = StyledStr::new();
        // `StyledStr`'s `fmt::Write` is infallible.
        self.write_command_sections(&mut before, T::CATEGORIES).ok();

        // clap can't regroup its subcommand listing, so the template lists `{options}` instead of
        // `{all-args}` and the menu goes in `{before-help}`.
        let header = styles.get_header();
        let template = format!(
            "{{name}} {{version}}\n{{author}}\n{{about}}\n\n{{usage-heading}}\n  \
             {{usage}}\n\n{{before-help}}{header}Options:{header:#}\n{{options}}"
        );

        self.before_help(before).help_template(template)
    }
}
