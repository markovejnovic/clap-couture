//! Beautiful, categorized `--help` for [`clap`] CLIs.
//!
//! # Example
//!
//! ```
//! use clap::{Parser, Subcommand};
//! use clap_couture::Couture;
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
//!     /// Search your history
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
//! `tool <cmd> --help` are unaffected — only the help *listing* changes.

mod render;

use clap::{Command, builder::StyledStr};
pub use clap_couture_derive::Couture;
use render::render_groups;

/// A clap subcommand name.
///
/// If you have a CLI that supports `app foo`, `app bar`, this refers to `foo` and `bar`.
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
///
/// You can create arbitrary categories, associate them to commands via
/// [`Couture::CATEGORIES`], and then reference them.
pub struct Category {
    /// A description you can provide to your categories. This gets rendered columnar.
    pub description: Option<&'static str>,

    /// Stable identifier that can be referenced.
    ///
    /// `#[category("...")]` references this field.
    pub label: &'static str,

    /// The user-facing heading. If this is not provided, then [`self`] will use [`Self::label`]
    /// and display that to the user.
    pub title: Option<&'static str>,
}

/// A static map from subcommand name to the category it belongs to.
#[derive(Clone, Copy)]
pub struct CommandCategoryMap(&'static [(CommandName, Category)]);

impl CommandCategoryMap {
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

/// Implemented by `#[derive(Couture)]` to expose a type's command categories.
///
/// You can implement this manually, but we recommend using `clap-couture-derive`
/// to derive it automatically.
pub trait Couture {
    /// Each command paired with the category it belongs to.
    ///
    /// Categories render in first-appearance order; a category's `title` and
    /// `description` come from its first pair (later duplicates, and `None`s, are
    /// ignored — so an inherited command can carry just a label while the command
    /// that owns the category supplies the metadata).
    const CATEGORIES: CommandCategoryMap;
}

/// Extension trait adding categorized help to a [`clap::Command`].
pub trait CommandExt {
    /// Attach couture to the given command.
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
    ///     /// Search your history
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
    fn with_couture<T>(mut self) -> Self
    where
        T: Couture,
    {
        let styles = self.get_styles().clone();

        // Snapshot visible subcommands before we start mutating `self`.
        let commands: Vec<(String, Option<String>)> = self
            .get_subcommands()
            .filter(|c| !c.is_hide_set())
            .map(|c| (c.get_name().to_owned(), c.get_about().map(ToString::to_string)))
            .collect();

        let mut before = String::new();
        render_groups(&mut before, &commands, T::CATEGORIES, &styles);
        self = self.before_help(StyledStr::from(before));

        // clap's `{options}` tag renders the option rows but not their heading
        // (only `{all-args}` does, and that would re-add the flat command list),
        // so we embed a styled "Options:" heading in the template ourselves —
        // its ANSI is stripped alongside everything else when colour is off.
        let header = styles.get_header();
        self = self.help_template(format!(
            "{{name}} {{version}}\n{{author}}\n{{about}}\n\n{{usage-heading}}\n  \
             {{usage}}\n\n{{before-help}}{header}Options:{header:#}\n{{options}}"
        ));

        self
    }
}
