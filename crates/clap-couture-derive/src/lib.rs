//! Derive macro for [`clap-couture`](https://docs.rs/clap-couture).

mod clap_compat;
mod couture_impl;
mod input;
mod literals;

use darling::FromDeriveInput as _;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{DeriveInput, parse_macro_input};

use crate::{couture_impl::CoutureImpl, input::CoutureInput};

/// Enable [`clap-couture`](https://docs.rs/clap-couture) on your clap subcommand enum.
///
/// - `#[couture(categories = { "key" = { title = "...", description = "..." }, ... })]` on the enum
///   declares the categories in display order. `title` and `description` are optional.
/// - `#[category("key")]` on a variant files it under that category. Naming a category the enum
///   neither declares nor inherits is a compile error, unless it declares and inherits none.
/// - With the `interactive` feature, `#[couture(prompt)]` or `#[couture(prompt = "...")]` on a
///   field asks for that arg when the user leaves it out.
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
///     "everyday" = { title = "Everyday", description = "Commands you'll reach for daily" },
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
/// #[couture(categories = { "admin" = { title = "Administration" } })]
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
