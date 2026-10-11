//! Derive macro for [`clap-couture`](https://docs.rs/clap-couture).

mod clap_compat;
mod couture_impl;
mod input;
mod text;

use darling::FromDeriveInput as _;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{DeriveInput, parse_macro_input};

use crate::{couture_impl::CoutureImpl, input::CoutureInput};

/// Enable [`clap-couture`](https://docs.rs/clap-couture) on your clap subcommand enum.
///
/// - `#[couture(categories = { "key" = { title = "...", description = "..." }, ... })]` on the enum
///   declares the categories in display order. `title` and `description` are optional. `title` is
///   the heading as printed, colon included (`"Deploy:"`, `"部署："`); without one, the heading is
///   the key and a colon.
/// - Similar to [`clap`](https://docs.rs/clap), you can pass arbitrary expressions to any field,
///   eg. `title = t!("deploy")`, which can enable localized text.
/// - `#[category("key")]` on a variant files it under that category. Naming a category the enum
///   neither declares nor inherits is a compile error, unless it declares and inherits none.
/// - A `#[command(flatten)]` variant or field brings its type's categories along, after the enum's
///   own. On a label both use, the enum's declared title and description win. A flattened
///   subcommand enum must derive `Couture` too; a flattened `Args` type need not.
/// - With the `interactive` feature, `#[couture(prompt)]` or `#[couture(prompt = "...")]` on a
///   field asks for that arg when the user leaves it out.
/// - With the `interactive-editor` feature, `#[couture(prompt, editor)]` asks in the user's
///   `$EDITOR` instead; `editor(extension = "md")` names the temp file's extension, and
///   `editor(initial = "...")` the text the file starts with when the arg has no default.
/// - With the `interactive` feature, `#[couture(confirm)]` or `#[couture(confirm = "...")]` on a
///   `bool` flag asks to go ahead after every prompt, unless the flag is passed. A no, or no
///   terminal to ask on, fails the parse. `{arg}` in the question stands for that arg's value.
/// - Add `secret` beside it, `#[couture(prompt, secret)]`, to ask for a `SecretString` without
///   echoing it. The arg then refuses a value from the command line; `secret(allow_argv)` allows
///   it.
/// - With the `interactive` feature, `#[couture(stdin)]` on a field takes what is piped into stdin
///   when the user leaves the arg out. One arg per command path may take it.
/// - `dash` gives `-` as the arg's value a meaning, and nothing else does: `prompt(dash)` asks for
///   the arg, and `stdin(dash)` reads all of stdin into it; with both, it asks at a terminal and
///   reads stdin otherwise. `prompt(dash, question = ...)` asks its own question.
/// - `#[couture(messages = { usage = ..., options = ..., none = ... })]` replaces the text couture
///   prints itself, eg. to localize it; see `clap_couture::Messages` for every key. Any of them may
///   be left out. It applies on the type you parse, or the one you pass to `with_couture`.
///
/// # Examples
///
/// Group subcommands under headings:
///
/// ```
/// use clap::{Parser, Subcommand};
/// use clap_couture::{Couture, CoutureParser as _};
///
/// #[derive(Parser, Couture)]
/// struct Cli {
///     #[command(subcommand)]
///     cmd: Cmd,
/// }
///
/// #[derive(Subcommand, Couture)]
/// #[couture(categories = {
///     "everyday" = { title = "Everyday:", description = "Commands you'll reach for daily" },
///     "admin" = {},
/// })]
/// enum Cmd {
///     /// Deploy the current project
///     #[category("everyday")]
///     Deploy,
///     /// Rotate the API keys
///     #[category("admin")]
///     RotateKeys,
///     /// Print the version
///     Version,
/// }
///
/// Cli::couture_command().print_help().unwrap();
/// ```
///
/// ```text
/// Usage:
///   tool <COMMAND>
///
///   version      Print the version
///
/// Everyday:  Commands you'll reach for daily
///   deploy       Deploy the current project
///
/// admin:
///   rotate-keys  Rotate the API keys
///
/// Options:
///   -h, --help  Print help
/// ```
///
/// A nested subcommand enum can file its variants under its parent's categories:
///
/// ```
/// use clap::Subcommand;
/// use clap_couture::Couture;
///
/// #[derive(Subcommand, Couture)]
/// #[couture(categories = { "admin" = { title = "Administration:" } })]
/// enum Cmd {
///     #[category("admin")]
///     #[command(subcommand)]
///     Keys(KeysCmd),
/// }
///
/// #[derive(Subcommand, Couture)]
/// #[couture(inherit = ["admin"])]
/// enum KeysCmd {
///     /// Rotate the API keys
///     #[category("admin")]
///     Rotate,
/// }
/// ```
///
/// Localize the text couture prints itself:
///
/// ```
/// use clap::Parser;
/// use clap_couture::{Couture, CoutureParser as _};
///
/// #[derive(Parser, Couture)]
/// #[couture(messages = { usage = "Verwendung:", options = "Optionen:" })]
/// struct Cli {
///     #[arg(long)]
///     verbose: bool,
/// }
///
/// let help = Cli::couture_command().render_help().to_string();
/// assert!(help.contains("Verwendung:"));
/// ```
///
/// With the interactive mode, you can add a couture prompt to ask the user for input interactively.
///
/// ```
/// use clap::Parser;
/// use clap_couture::{Couture, CoutureParser as _};
///
/// #[derive(Parser, Couture)]
/// struct Cli {
///     /// Region to deploy to
///     #[arg(long)]
///     #[couture(prompt)]
///     region: String,
///
///     #[arg(long)]
///     #[couture(prompt = "Tail the logs afterwards?")]
///     follow: bool,
/// }
/// ```
#[proc_macro_derive(Couture, attributes(category, couture))]
pub fn derive_couture(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    CoutureInput::from_derive_input(&input)
        .and_then(CoutureImpl::try_from)
        .map_or_else(darling::Error::write_errors, ToTokens::into_token_stream)
        .into()
}
