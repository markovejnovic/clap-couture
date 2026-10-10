//! `MESSAGES`: the text couture prints itself, as `#[couture(messages = { ... })]` sets it.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use serde::Deserialize;

use crate::text::TextExpr;

/// The messages a type sets; the rest keep `clap_couture::Messages::DEFAULT`'s.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Messages {
    none: Option<TextExpr>,
    options: Option<TextExpr>,
    usage: Option<TextExpr>,
}

impl ToTokens for Messages {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let set_messages =
            [("none", &self.none), ("options", &self.options), ("usage", &self.usage)]
                .into_iter()
                .filter_map(|(name, text)| {
                    let name = syn::Ident::new(name, proc_macro2::Span::call_site());
                    text.as_ref().map(|text| quote!(#name: #text))
                });
        tokens.extend(quote! {
            ::clap_couture::Messages { #(#set_messages,)* ..::clap_couture::Messages::DEFAULT }
        });
    }
}
