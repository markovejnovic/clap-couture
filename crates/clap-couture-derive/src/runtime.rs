//! How the generated code names the clap-couture crate.

use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::ToTokens;

/// The clap-couture crate in the generated code: an alias the impl's enclosing block binds to
/// `::clap_couture`, or to the `#[couture(crate = ...)]` path, so a renamed dependency works.
pub(crate) struct Runtime;

impl Runtime {
    /// The alias's name.
    pub(crate) const ALIAS: &str = "__clap_couture";
}

impl ToTokens for Runtime {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        Ident::new(Self::ALIAS, Span::call_site()).to_tokens(tokens);
    }
}
