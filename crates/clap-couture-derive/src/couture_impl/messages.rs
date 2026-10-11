//! `MESSAGES`: the text couture prints itself, as `#[couture(messages = { ... })]` sets it.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use syn::{
    Ident, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use super::container::{Entry, Keys};
use crate::{runtime::Runtime, text::TextExpr};

/// The messages a type sets, in written order; the rest keep `clap_couture::Messages::DEFAULT`'s.
pub(crate) struct Messages(Vec<(Ident, TextExpr)>);

impl Messages {
    /// Every key `messages = { ... }` takes, one per `clap_couture::Messages` builder.
    const KEYS: Keys = Keys(&[
        "arguments",
        "editor_empty",
        "editor_instructions",
        "editor_instructions_optional",
        "editor_not_utf8",
        "editor_returned_early",
        "editor_unreadable",
        "editor_waiting",
        "environment",
        "finish_hint",
        "none",
        "options",
        "selection_required",
        "usage",
    ]);
}

impl Parse for Messages {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Punctuated::<Entry, Token![,]>::parse_terminated(input)?
            .into_iter()
            .map(|Entry { key, value }| {
                Self::KEYS.check(&key)?;
                Ok((key, TextExpr::from(value)))
            })
            .collect::<syn::Result<_>>()
            .map(Self)
    }
}

impl ToTokens for Messages {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let setters = self.0.iter().map(|(key, text)| {
            let setter = format_ident!("with_{key}", span = key.span());
            quote!(.#setter(#text))
        });
        tokens.extend(quote!(#Runtime::Messages::DEFAULT #(#setters)*));
    }
}
