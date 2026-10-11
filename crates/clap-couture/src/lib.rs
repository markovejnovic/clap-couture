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

extern crate alloc;

#[cfg(feature = "interactive")]
pub mod interactive;
mod maybe_couture;
mod parser;
mod render;
mod text;

use alloc::borrow::Cow;

use clap::{Command, builder::StyledStr};
pub use clap_couture_derive::Couture;
#[doc(hidden)]
pub use maybe_couture::{MaybeCouture, NotCouture};
pub use parser::CoutureParser;
use render::CommandRenderExt as _;
pub use text::Text;

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

/// The text couture prints itself, for localizing it.
///
/// Set any of it with `#[couture(messages = { ... })]` on the type you parse; each value takes an
/// expression like [`Text`]. A heading message includes its trailing colon, since languages
/// punctuate it differently: `Usage:`, `用法：`, `Utilisation :`.
///
/// # Example
#[cfg_attr(feature = "interactive", doc = "```")]
#[cfg_attr(not(feature = "interactive"), doc = "```ignore")]
/// use clap::{Parser, Subcommand, ValueEnum};
/// use clap_couture::{Couture, CoutureParser as _};
///
/// #[derive(Parser, Couture)]
/// #[command(name = "tool")]
/// #[couture(messages = {
///     none = "无",
///     options = "选项：",
///     usage = "用法：",
/// })]
/// struct Cli {
///     /// 部署区域
///     #[arg(long, value_enum)]
///     #[couture(prompt)]
///     region: Option<Region>,
///
///     #[command(subcommand)]
///     cmd: Cmd,
/// }
///
/// #[derive(Clone, ValueEnum)]
/// enum Region {
///     Eu,
///     Us,
/// }
///
/// #[derive(Subcommand, Couture)]
/// #[couture(categories = { "deploy" = { title = "部署：" } })]
/// enum Cmd {
///     /// 部署当前项目
///     #[category("deploy")]
///     Ship,
/// }
///
/// Cli::couture_command().print_help().unwrap();
/// ```
/// 
/// `tool --help` renders
/// ```text
/// 用法：
///   tool [OPTIONS] <COMMAND>
///
/// 部署：
///   ship  部署当前项目
///
/// 选项：
///       --region <REGION>  部署区域 [possible values: eu, us]
///   -h, --help             Print help
/// ```
/// 
/// and `tool ship`, run on a terminal without `--region`, asks for the optional region with
/// `none` as its last choice. Picking it leaves `region` unset. With the
/// `interactive-cliclack` backend:
/// ```text
/// ◆  部署区域
/// │  ● eu
/// │  ○ us
/// │  ○ 无
/// └
/// ```
/// 
/// `Print help` and `[possible values: ...]` are clap's own text; for the help flag, disable
/// clap's and add your own.
#[derive(Clone, Copy, Debug)]
pub struct Messages {
    /// Reported when a `#[couture(prompt, editor)]` arg that is required or has a default is
    /// answered with an empty file.
    /// The default is `"Aborting due to an empty answer."`.
    pub editor_empty: Text,
    /// The last header line of the file of a `#[couture(prompt, editor)]` arg that is required or
    /// has a default. The default
    /// is `"These lines are removed. Leave the file empty to cancel."`.
    pub editor_instructions: Text,
    /// The last header line of the file of an optional `#[couture(prompt, editor)]` arg with no
    /// default. The default
    /// is `"These lines are removed. Leave the file empty to leave it unset."`.
    pub editor_instructions_optional: Text,
    /// The error a `#[couture(prompt, editor)]` file reopens with when it was saved in an
    /// encoding that can't be read. The default is `"The file is not UTF-8. Save it as UTF-8."`.
    pub editor_not_utf8: Text,
    /// Asked when the editor returned right away and left the file as it was, as `code` does
    /// without `--wait`. The default is `"Your editor returned right away. Done editing the
    /// file?"`.
    pub editor_returned_early: Text,
    /// Put before the setting that makes a known editor wait, on a line after
    /// `editor_returned_early`. The default is `"To make it wait, set"`.
    pub editor_wait_hint: Text,
    /// Shown while the editor is open. The default is
    /// `"hint: Waiting for your editor to close the file..."`.
    pub editor_waiting: Text,
    /// The select option that leaves an optional prompted arg unset. The default is `"(none)"`.
    pub none: Text,
    /// The heading above the options in `--help`. The default is `"Options:"`.
    pub options: Text,
    /// The heading above the usage in `--help`. The default is `"Usage:"`.
    pub usage: Text,
}

impl Messages {
    /// clap's English wording.
    pub const DEFAULT: Self = Self {
        editor_empty: Text::Literal("Aborting due to an empty answer."),
        editor_instructions: Text::Literal(
            "These lines are removed. Leave the file empty to cancel.",
        ),
        editor_instructions_optional: Text::Literal(
            "These lines are removed. Leave the file empty to leave it unset.",
        ),
        editor_not_utf8: Text::Literal("The file is not UTF-8. Save it as UTF-8."),
        editor_returned_early: Text::Literal(
            "Your editor returned right away. Done editing the file?",
        ),
        editor_wait_hint: Text::Literal("To make it wait, set"),
        editor_waiting: Text::Literal("hint: Waiting for your editor to close the file..."),
        none: Text::Literal("(none)"),
        options: Text::Literal("Options:"),
        usage: Text::Literal("Usage:"),
    };
}

/// Defines a category of similar commands.
pub struct Category {
    /// Text printed beside the heading, if any.
    pub description: Option<Text>,

    /// Key that `#[category("...")]` refers to.
    pub label: &'static str,

    /// Heading shown to the user, colon included; falls back to [`Self::label`] and a colon.
    pub title: Option<Text>,
}

impl Category {
    /// The user-facing heading: [`Self::title`], falling back to [`Self::label`] and a colon.
    pub(crate) fn heading(&self) -> Cow<'static, str> {
        self.title.as_ref().map_or_else(|| Cow::Owned(format!("{}:", self.label)), Text::get)
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
    declared_categories: &'static [Category],
    /// The maps of the types flattened into this map's command, in declared order.
    flattened_maps: &'static [Self],
    /// The `(command, category)` pairs this map files itself.
    own_entries: &'static [(CommandName, Category)],
}

impl CommandCategoryMap {
    /// A map with no entries.
    pub const EMPTY: Self =
        Self { declared_categories: &[], flattened_maps: &[], own_entries: &[] };

    /// This map, then its flattened maps, depth first.
    fn depth_first(self) -> DepthFirstMaps {
        DepthFirstMaps { maps_to_walk: Vec::from([self]) }
    }

    /// The distinct categories, in first-appearance order: a map's declared categories, then
    /// those of its entries, then its flattened maps'.
    pub(crate) fn distinct(self) -> impl Iterator<Item = &'static Category> {
        let mut seen_labels = Vec::new();
        self.depth_first()
            .flat_map(|map| {
                map.declared_categories.iter().chain(map.own_entries.iter().map(|entry| &entry.1))
            })
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
        CommandCategoryEntries {
            current_map_entries: [].iter(),
            remaining_maps: self.depth_first(),
        }
    }

    /// Wrap a static slice of `(command, category)` pairs.
    #[must_use]
    pub const fn new(entries: &'static [(CommandName, Category)]) -> Self {
        Self { declared_categories: &[], flattened_maps: &[], own_entries: entries }
    }

    /// Declare categories up front, so they head the listing and win over a flattened map's
    /// same-label category even when this map files no command under them.
    #[must_use]
    pub const fn with_declared_categories(self, declared_categories: &'static [Category]) -> Self {
        Self {
            declared_categories,
            flattened_maps: self.flattened_maps,
            own_entries: self.own_entries,
        }
    }

    /// Add the maps of the types flattened into this command, after this map's own entries.
    #[must_use]
    pub const fn with_flattened_maps(self, flattened_maps: &'static [Self]) -> Self {
        Self {
            declared_categories: self.declared_categories,
            flattened_maps,
            own_entries: self.own_entries,
        }
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
    current_map_entries: core::slice::Iter<'static, (CommandName, Category)>,
    /// The maps after the current one.
    remaining_maps: DepthFirstMaps,
}

impl Iterator for CommandCategoryEntries {
    type Item = &'static (CommandName, Category);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.current_map_entries.next() {
                return Some(entry);
            }
            self.current_map_entries = self.remaining_maps.next()?.own_entries.iter();
        }
    }
}

/// A [`CommandCategoryMap`] and its flattened maps, depth first.
#[derive(Clone)]
struct DepthFirstMaps {
    /// The maps still to walk, the next one last.
    maps_to_walk: Vec<CommandCategoryMap>,
}

impl Iterator for DepthFirstMaps {
    type Item = CommandCategoryMap;

    fn next(&mut self) -> Option<Self::Item> {
        let map = self.maps_to_walk.pop()?;
        self.maps_to_walk.extend(map.flattened_maps.iter().rev());
        Some(map)
    }
}

/// A type's command categories, usually from `#[derive(Couture)]` on a subcommand enum.
pub trait Couture {
    /// Each command paired with the category it belongs to.
    const CATEGORIES: CommandCategoryMap = CommandCategoryMap::EMPTY;

    /// The text couture prints itself, from `#[couture(messages = { ... })]`.
    const MESSAGES: Messages = Messages::DEFAULT;

    /// The args marked `#[couture(prompt)]`, and where they sit.
    #[cfg(feature = "interactive")]
    #[doc(hidden)]
    const PROMPTED_ARGS: interactive::PromptedArgTree = interactive::PromptedArgTree::EMPTY;
}

/// Extension trait adding categorized help to a [`clap::Command`].
pub trait CommandExt {
    /// Group this command's `--help` subcommand listing by `T`'s categories, with `T`'s
    /// [`Messages`].
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
        // `{all-args}` and the menu goes in `{before-help}`. The headings are written out rather
        // than clap's `{usage-heading}`, which is always English.
        let (header, usage) = (styles.get_header(), styles.get_usage());
        let (options_heading, usage_heading) = (T::MESSAGES.options.get(), T::MESSAGES.usage.get());
        let template = format!(
            "{{name}} {{version}}\n{{author}}\n{{about}}\n\n{usage}{usage_heading}{usage:#}\n  \
             {{usage}}\n\n{{before-help}}{header}{options_heading}{header:#}\n{{options}}"
        );

        self.before_help(before).help_template(template)
    }
}
