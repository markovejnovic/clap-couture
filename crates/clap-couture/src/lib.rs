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
//! A `#[command(flatten)]` variant is no subcommand itself, so it takes no `#[category("...")]`;
//! file the flattened enum's variants instead:
//!
//! ```compile_fail
//! use clap::Subcommand;
//! use clap_couture::Couture;
//!
//! #[derive(Subcommand, Couture)]
//! #[couture(categories = { "account" = {} })]
//! enum Cmd {
//!     #[category("account")]
//!     #[command(flatten)]
//!     Account(Account),
//! }
//!
//! #[derive(Subcommand, Couture)]
//! enum Account {
//!     Login,
//! }
//! ```
//!
//! The subcommands stay visible to clap, so shell completions and
//! `tool <cmd> --help` are unaffected -- only the help *listing* changes.

#[cfg(feature = "interactive")]
pub mod interactive;
mod maybe_couture;
mod parser;
mod render;

use clap::{Command, builder::StyledStr};
pub use clap_couture_derive::Couture;
#[doc(hidden)]
pub use maybe_couture::{MaybeCouture, NotCouture};
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
///
/// Besides its own entries, a map holds the maps of the types `#[command(flatten)]` merges into
/// its command, so their subcommands keep their categories.
#[derive(Clone, Copy)]
pub struct CommandCategoryMap {
    /// The categories this map's command declares, in heading order, including ones only
    /// flattened commands are filed under. They win over a flattened map's same-label category.
    categories: &'static [Category],
    /// The maps of the flattened types, in declared order.
    flattened: &'static [Self],
    /// The `(command, category)` pairs this map files itself.
    own: &'static [(CommandName, Category)],
}

impl CommandCategoryMap {
    /// A map with no entries.
    pub const EMPTY: Self = Self { categories: &[], flattened: &[], own: &[] };

    /// This map, then its flattened maps, depth first.
    fn depth_first(self) -> DepthFirstMaps {
        DepthFirstMaps { pending: Vec::from([self]) }
    }

    /// The distinct categories, in first-appearance order: a map's declared categories, then
    /// those of its entries, then its flattened maps'.
    pub(crate) fn distinct(self) -> impl Iterator<Item = &'static Category> {
        let mut seen_labels = Vec::new();
        self.depth_first()
            .flat_map(|map| map.categories.iter().chain(map.own.iter().map(|entry| &entry.1)))
            .filter(move |category| {
                let is_first = !seen_labels.contains(&category.label);
                if is_first {
                    seen_labels.push(category.label);
                }
                is_first
            })
    }

    /// The category assigned to `name`, if any.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&'static Category> {
        self.iter().find(|entry| entry.0.as_str() == name).map(|entry| &entry.1)
    }

    /// Iterate the `(command, category)` pairs: the map's own in declared order, then each
    /// flattened map's.
    #[must_use]
    pub fn iter(&self) -> CommandCategoryEntries {
        CommandCategoryEntries { current: [].iter(), maps: self.depth_first() }
    }

    /// Wrap a static slice of `(command, category)` pairs.
    #[must_use]
    pub const fn new(entries: &'static [(CommandName, Category)]) -> Self {
        Self { categories: &[], flattened: &[], own: entries }
    }

    /// Declare categories up front, so they head the listing and win over a flattened map's
    /// same-label category even when this map files no command under them.
    #[must_use]
    pub const fn with_categories(self, categories: &'static [Category]) -> Self {
        Self { categories, flattened: self.flattened, own: self.own }
    }

    /// Add the maps of the types flattened into this command, after this map's own entries.
    #[must_use]
    pub const fn with_flattened(self, flattened: &'static [Self]) -> Self {
        Self { categories: self.categories, flattened, own: self.own }
    }
}

impl IntoIterator for &CommandCategoryMap {
    type IntoIter = CommandCategoryEntries;
    type Item = &'static (CommandName, Category);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// The `(command, category)` pairs of a [`CommandCategoryMap`], depth first.
#[derive(Clone)]
pub struct CommandCategoryEntries {
    /// The entries of the map being walked.
    current: core::slice::Iter<'static, (CommandName, Category)>,
    /// The maps still to walk.
    maps: DepthFirstMaps,
}

impl Iterator for CommandCategoryEntries {
    type Item = &'static (CommandName, Category);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.current.next() {
                return Some(entry);
            }
            self.current = self.maps.next()?.own.iter();
        }
    }
}

/// A [`CommandCategoryMap`] and its flattened maps, depth first.
#[derive(Clone)]
struct DepthFirstMaps {
    /// The maps still to walk, the next one last.
    pending: Vec<CommandCategoryMap>,
}

impl Iterator for DepthFirstMaps {
    type Item = CommandCategoryMap;

    fn next(&mut self) -> Option<Self::Item> {
        let map = self.pending.pop()?;
        self.pending.extend(map.flattened.iter().rev());
        Some(map)
    }
}

/// A type's command categories, usually from `#[derive(Couture)]` on a subcommand enum.
pub trait Couture {
    /// Each command paired with the category it belongs to.
    const CATEGORIES: CommandCategoryMap = CommandCategoryMap::EMPTY;

    /// The args marked `#[couture(prompt)]`, and where they sit.
    #[cfg(feature = "interactive")]
    #[doc(hidden)]
    const PROMPTED_ARGS: interactive::PromptedArgTree = interactive::PromptedArgTree::EMPTY;
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
