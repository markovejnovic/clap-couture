//! **Haute couture for your command line.**
//!
//! [`clap_couture`](self) is a semi-opinionated extension set for
//! [`clap`](https://crates.io/crates/clap) that:
//!
//!   - Organizes CLI subcommands in **categories**, improving readability and making your CLIs less
//!     intimidating.
//!   - **Interactive input** for flags.
//!   - **Markdown** support.
//!
//! # Getting Started
//!
//! This section will provide you with a tour guide through [`clap_couture`](self)'s features. For
//! some more in-depth examples, please have a look at the
//! [examples](https://github.com/markovejnovic/clap-couture/tree/main/crates/clap-couture/examples)
//! directory. I've put some good love into making those examples as helpful as I can.
//!
//! ## Categories
//!
//! As CLIs grow, the total set of subcommands grows very quickly, and a first-time user running
//! your application `foobar --help`, sees a massive wall of text. Making your application less
//! intimidating makes it more accessible to the beginner user, which makes it more inviting and
//! friendly.
//!
//! <style>
//!   .couture-light { display: none; }
//!   :root[data-theme="light"] .couture-light { display: revert; }
//!   :root[data-theme="light"] .couture-dark { display: none; }
//!   .docblock img.couture-dark, .docblock img.couture-light {
//!     max-width: 100%;
//!     height: auto;
//!     border-radius: var(--code-block-border-radius, 6px);
//!   }
//! </style>
//!
//! With clap:
#![cfg_attr(doc, doc = include_str!("../docs/01-orbit-disorganized.html"))]
//! With clap-couture:
#![cfg_attr(doc, doc = include_str!("../docs/01-orbit-organized.html"))]
//! To get started with this feature, take your existing code:
//!
//! ```
//! # use clap::{Parser, Subcommand};
//! #[derive(Parser)]
//! struct Cli {
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[derive(Subcommand)]
//! enum Cmd {
//!     /// Ship the current project.
//!     Deploy,
//!     /// Sign in to your account.
//!     Login,
//!     /// Tail live logs.
//!     Logs,
//!     /// Roll back to a previous release.
//!     Rollback,
//!     /// Search your deployment history.
//!     Search,
//!     /// Manage environment secrets.
//!     Secrets,
//!     /// Show recent activity.
//!     Status,
//!     /// Sync settings across machines.
//!     Sync,
//! }
//! ```
//!
//! Derive [`Couture`](derive@Couture) on your command and organize your commands in different
//! categories.
//!
//! ```
//! # use clap::{Parser, Subcommand};
//! # use clap_couture::Couture;
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[derive(Subcommand, Couture)]
//! #[couture(categories = { "everyday" = {}, "deploy" = {}, "account" = {} })]
//! enum Cmd {
//!     /// Ship the current project.
//!     #[category("deploy")]
//!     Deploy,
//!     /// Sign in to your account.
//!     #[category("account")]
//!     Login,
//!     /// Tail live logs.
//!     #[category("everyday")]
//!     Logs,
//!     /// Roll back to a previous release.
//!     #[category("deploy")]
//!     Rollback,
//!     /// Search your deployment history.
//!     #[category("everyday")]
//!     Search,
//!     /// Manage environment secrets.
//!     #[category("deploy")]
//!     Secrets,
//!     /// Show recent activity.
//!     #[category("everyday")]
//!     Status,
//!     /// Sync settings across machines.
//!     #[category("account")]
//!     Sync,
//! }
//! ```
//!
//! You need to call [`CoutureParser::couture_parse`] instead of
//! [`Parser::parse`](clap::Parser::parse), and you'll get the organized `--help` menu.
//!
//! ## Markdown Support
//!
//! [`clap_couture`](self) also has support for markdown with the `markdown` feature flag.
//!
//! ```
//! use clap::{Parser, Subcommand};
//! use clap_couture::Couture;
//! # use clap_couture::CoutureParser as _;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[derive(Subcommand, Couture)]
//! enum Cmd {
//!     /// Ship the **current** project.
//!     Deploy,
//!     /// Sign in to *your* account.
//!     Login,
//! }
//! # let help = Cli::couture_command().render_help().to_string();
//! # assert!(help.contains("Ship the current project"));
//! # assert!(help.contains("Sign in to your account"));
//! ```
//!
//! This results in:
#![cfg_attr(doc, doc = include_str!("../docs/02-markdown-simple.html"))]
//! ## Interactive Input
//!
//! Finally, `clap-couture` lets you easily add interactive input, straight into your clap structs.
//! Often enough, you might want to express the idea of "if the user doesn't provide a CLI argument,
//! we should ask the user for input on stdin". With [`clap_couture`](self), that's just a tiny
//! macro away.
//!
//! You can decorate your argument flag fields:
//!
//! ```
//! use clap::{Parser, ValueEnum};
//! use clap_couture::{Couture, CoutureParser as _};
//! # use clap_couture::CoutureParser;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt = "Which app?")]
//!     app: String,
//!
//!     #[arg(long)]
//!     #[couture(prompt = "Tail the logs afterwards?")]
//!     follow: bool,
//!
//!     #[arg(long, value_enum)]
//!     #[couture(prompt = "Which region?")]
//!     region: Region,
//!
//!     #[arg(long, default_value_t = 2)]
//!     #[couture(prompt = "How many replicas?")]
//!     replicas: u16,
//! }
//!
//! #[derive(Clone, Debug, PartialEq, ValueEnum)]
//! enum Region {
//!     Apac,
//!     Eu,
//!     Us,
//! }
//!
//! # // A doctest has no terminal to prompt on, so pass every arg instead.
//! # impl Cli {
//! #     fn couture_parse() -> Self {
//! #         <Self as CoutureParser>::couture_parse_from([
//! #             "orbit-deploy", "--app", "web", "--follow", "--region", "eu",
//! #         ])
//! #     }
//! # }
//! let cli = Cli::couture_parse();
//! # assert_eq!(cli.app, "web");
//! # assert!(cli.follow);
//! # assert_eq!(cli.region, Region::Eu);
//! # assert_eq!(cli.replicas, 2);
//! ```
//!
//! This results in support for interactive input:
//!
//! <img class="couture-dark" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/03-interactive-dark.gif" alt="running orbit-deploy with no flags: it asks for the app as text, whether to tail the logs as yes/no, and the region from a list, then asks for the replicas again after an answer that isn't a number" width="846">
//! <img class="couture-light" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/03-interactive-light.gif" alt="running orbit-deploy with no flags: it asks for the app as text, whether to tail the logs as yes/no, and the region from a list, then asks for the replicas again after an answer that isn't a number" width="846">
//!
//! ### Neat features
//!
//! - `#[couture(prompt)]` asks the field's doc comment, and `#[couture(prompt = "...")]` asks
//!   whatever you write instead.
//! - `#[couture(prompt)]` on an arg with a `default_value` prompts the user, pre-filling the input
//!   form.
//! - Couture parses rich types the same way `clap` does in interactive mode. Types like `u16` (or
//!   any [`FromStr`](core::str::FromStr) type, really) get correctly parsed, even in interactive
//!   mode.
//! - `#[couture(prompt)]` will avoid inquiring about fields the user passed either through the CLI
//!   flags or the environment.
//! - `#[couture(prompt)]` only queries the input field if there's a PTY to query on. If the user is
//!   not in a terminal (eg. scripts, CI), couture will skip interactive prompts.
//!
//! ### Input Types
//!
//! [`clap_couture`](self) has support for different types of fields:
//!
//! | Field Type                | Example                 | Prompt         | Example                                         |
//! |---------------------------|-------------------------|----------------|-------------------------------------------------|
//! | `bool`                    | `force: bool`           | `[y/N]`        | Do you want to force input [y/N]: **y**         |
//! | Possible values           | `region: RegionEnum`    | Picker         | Which region (eu)? **us**                       |
//! | Arbitrary types           | `count: u16`            | Free text      | How many: **32**                                |
//! | `Option<T>`               | `fruit: Option<String>` | Optional input | Which fruit (none): **`<enter>`**               |
//! | `Vec<T>`, possible values | `envs: Vec<Env>`        | Multi-picker   | Which envs? **dev, stage**                      |
//! | `Vec<T>`, arbitrary type  | `tags: Vec<String>`     | Repeated input | Which tags `(<enter> to finish)`: **`<enter>`** |
//! | `SecretString`, `secret`  | `token: SecretString`   | Masked input   | Deploy token: **▪▪▪▪▪▪▪**                       |
//!
//! ### Confirmation
//!
//! `#[couture(confirm = "...")]` on a `bool` flag asks before going ahead. A yes sets the flag, a
//! no exits with status 1. Passing the flag skips the question, and without a terminal it must be
//! passed, so a parse that returns always has the flag set. `{arg}` in the question stands for
//! that arg's value; `{{` and `}}` are literal braces.
//!
//! ```
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser as _};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     name: String,
//!
//!     #[arg(long)]
//!     #[couture(confirm = "Delete {name} for good?")]
//!     force: bool,
//! }
//!
//! let cli = Cli::couture_parse_from(["delete", "--name", "web", "--force"]);
//! ```
//!
//! An example of this interaction is:
//!
//! <img class="couture-dark" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/05-interactive-confirm-dark.gif" alt="running orbit-destroy with no flags: it asks for the app and the region, then asks whether to destroy web in eu, and goes ahead on a yes" width="623">
//! <img class="couture-light" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/05-interactive-confirm-light.gif" alt="running orbit-destroy with no flags: it asks for the app and the region, then asks whether to destroy web in eu, and goes ahead on a yes" width="623">
//!
//! See [`examples/05-interactive-confirm.rs`](https://github.com/markovejnovic/clap-couture/blob/main/crates/clap-couture/examples/05-interactive-confirm.rs).
//!
//! ### Secrets
//!
//! `#[couture(prompt, secret)]` on a [`SecretString`](interactive::secrecy::SecretString) field
//! asks without echoing the answer, just like `sudo`. Note that this flag is refused on the CLI,
//! by default, as that is considered unsafe. `secret(allow_argv)` allows it.
//!
//! A value from its env var is used without asking, and `--help` hides it.
//!
//! ```no_run
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser as _, interactive::secrecy::SecretString};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Deploy token.
//!     #[arg(long, env = "ORBIT_TOKEN")]
//!     #[couture(prompt, secret)]
//!     token: SecretString,
//! }
//!
//! let cli = Cli::couture_parse();
//! ```
//!
//! An example of this interaction is:
//!
//! <img class="couture-dark" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/07-interactive-secret-dark.gif" alt="running orbit-deploy with no flags: it asks for the app, then for the deploy token, showing only a mask as it is typed, and deploys" width="623">
//! <img class="couture-light" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/07-interactive-secret-light.gif" alt="running orbit-deploy with no flags: it asks for the app, then for the deploy token, showing only a mask as it is typed, and deploys" width="623">
//!
//! See [`examples/07-interactive-secret.rs`](https://github.com/markovejnovic/clap-couture/blob/main/crates/clap-couture/examples/07-interactive-secret.rs).
//!
//! ### Piped input
//!
//! `#[couture(accept_stdin)]` lets an arg come from a pipe: `echo hi | post` works like `post
//! --message hi`. Stdin is read only when it is a pipe and the arg was left out, so a terminal, a
//! file or `/dev/null` on stdin never blocks the parse. Beside `prompt`, the arg is asked when
//! nothing is piped; beside `secret`, it works like `docker login --password-stdin`.
//!
//! ```
//! use clap::Parser;
//! use clap_couture::{Couture, CoutureParser as _};
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     /// Message to post.
//!     #[arg(long)]
//!     #[couture(prompt, accept_stdin)]
//!     message: String,
//! }
//!
//! let cli = Cli::couture_parse_from(["post", "--message", "hi"]);
//! # assert_eq!(cli.message, "hi");
//! ```
//!
//! An example of this interaction is:
//!
//! <img class="couture-dark" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/10-interactive-stdin-dark.gif" alt="piping release notes into orbit-release, which takes them without asking, then running orbit-release alone, which asks for the notes" width="623">
//! <img class="couture-light" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/10-interactive-stdin-light.gif" alt="piping release notes into orbit-release, which takes them without asking, then running orbit-release alone, which asks for the notes" width="623">
//!
//! See [`examples/10-interactive-stdin.rs`](https://github.com/markovejnovic/clap-couture/blob/main/crates/clap-couture/examples/10-interactive-stdin.rs).
//!
//! Only one arg on a command may take stdin, as stdin can only be read once. See
//! [the `interactive` module](interactive#reading-piped-stdin) for the details.
//!
//! ### Custom interactive input
//!
//! Some questions only make sense after the parse. [`CoutureParser::prompter`] asks them in the
//! same UI, styles and behavior as clap-couture does.
//! [`Prompter::secret`](interactive::Prompter::secret) asks for a secret without echoing it, and
//! with `interactive-editor`, [`Prompter::editor`](interactive::Prompter::editor) asks in the
//! user's `$EDITOR`.
//!
//! ```
//! use clap::{Parser, ValueEnum};
//! use clap_couture::{Couture, CoutureParser as _};
//!
//! #[derive(Parser, Couture)]
//! struct Cli;
//!
//! #[derive(Clone, ValueEnum)]
//! enum Engine {
//!     Postgres,
//!     Sqlite,
//! }
//!
//! # fn main() -> Result<(), clap_couture::interactive::PromptError> {
//! if let Some(prompter) = Cli::prompter()
//!     && prompter.confirm("Add a database?", true)?
//! {
//!     let engine = prompter.select::<Engine>("Which engine?")?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! An example of this interaction is:
//!
//! <img class="couture-dark" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/08-interactive-prompter-dark.gif" alt="running orbit-create with no flags: it asks for the app, then whether to add a database, and on a yes, which engine" width="623">
//! <img class="couture-light" src="https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/08-interactive-prompter-light.gif" alt="running orbit-create with no flags: it asks for the app, then whether to add a database, and on a yes, which engine" width="623">
//!
//! See [`examples/08-interactive-prompter.rs`](https://github.com/markovejnovic/clap-couture/blob/main/crates/clap-couture/examples/08-interactive-prompter.rs).
//!
//! ### Backends
//!
//! You can pick from three interactive backends:
//!
//!   - [`interactive-cliclack`](interactive::Cliclack) uses the [cliclack](https://docs.rs/cliclack/latest/cliclack/)
//!     crate to ask the user for input.
//!   - [`interactive-inquire`](interactive::Inquire) uses the [inquire](https://docs.rs/inquire/latest/inquire/)
//!     crate to ask the user for input.
//!   - [`interactive-dialoguer`](interactive::Dialoguer) uses the [dialoguer](https://docs.rs/dialoguer/latest/dialoguer/)
//!     crate to ask the user for input.
//!
//! # Internationalization
//!
//! `clap-couture` allows you to accept *any* expression instead of plain strings, which can be
//! useful to localize with something like [fluent](https://projectfluent.org/).
//!
//! ```rust,ignore
//! #[couture(messages = {
//!     none = fl!("none-opt"),
//!     options = fl!("options-opt"),
//!     usage = fl!("usage-opt"),
//! })]
//! struct Cli {
//!     #[arg(long, value_enum)]
//!     #[couture(prompt)]
//!     region: Option<Region>,
//!
//!     #[command(subcommand)]
//!     cmd: Cmd,
//! }
//!
//! #[couture(categories = { "deploy" = { title = fl!("deploy-opt") } })]
//! enum Cmd {
//!     #[category("deploy")]
//!     Ship,
//! }
//!
//! Cli::couture_command().print_help().unwrap();
//! ```
//!
//! For a full example, see
//! [`examples/04-interactive-i18n.rs`](https://github.com/markovejnovic/clap-couture/blob/main/crates/clap-couture/examples/04-interactive-i18n.rs).

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/markovejnovic/clap-couture/main/assets/favicon.svg"
)]

extern crate alloc;

#[cfg(feature = "interactive-editor")]
mod fs;
#[cfg(feature = "interactive")]
pub mod interactive;
mod maybe_couture;
#[cfg(feature = "interactive")]
mod os;
mod parser;
mod render;
#[cfg(feature = "interactive-editor")]
mod strings;
mod text;

use alloc::borrow::Cow;
use core::fmt::Write as _;

use clap::{Command, builder::StyledStr};
#[cfg(feature = "derive")]
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
    /// Shown while the editor is open; empty shows nothing. The default is
    /// `"hint: Waiting for your editor to close the file..."`.
    pub editor_waiting: Text,
    pub environment: Text,
    /// The hint beside each prompt for one more value of a multi-value arg, telling that an empty
    /// answer finishes the list. The default is `"(<enter> to finish)"`.
    pub finish_hint: Text,
    /// The select option that leaves an optional prompted arg unset. The default is `"(none)"`.
    pub none: Text,
    /// The heading above the options in `--help`. The default is `"Options:"`.
    pub options: Text,
    /// What a required multi-select tells the user when nothing is selected. The default is
    /// `"Select at least one option"`.
    pub selection_required: Text,
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
        editor_waiting: Text::Literal("hint: Waiting for your editor to close the file..."),
        environment: Text::Literal("Environment:"),
        finish_hint: Text::Literal("(<enter> to finish)"),
        none: Text::Literal("(none)"),
        options: Text::Literal("Options:"),
        selection_required: Text::Literal("Select at least one option"),
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

    /// The args marked `#[couture(prompt)]` or `#[couture(confirm)]`, and where they sit.
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
    /// Leaves `#[couture(secret)]` args unguarded; take the command from
    /// [`CoutureParser::couture_command`] to refuse them on the command line.
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
        let mut template =
            format!("{{about-with-newline}}\n{usage}{usage_heading}{usage:#}\n  {{usage}}\n\n");
        // `String`'s `fmt::Write` is infallible.
        writeln!(template, "{{before-help}}{header}{options_heading}{header:#}").ok();
        template.push_str("{options}{after-help}");

        // clap follows a set `before_help` with a blank line, even an empty one, so a command
        // without subcommands would get a gap where the menu goes.
        let command = if before.to_string().is_empty() { self } else { self.before_help(before) };
        command.help_template(template)
    }
}
