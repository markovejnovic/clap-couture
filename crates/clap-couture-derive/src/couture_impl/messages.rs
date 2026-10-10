//! `MESSAGES`: the text couture prints itself, as `#[couture(messages = { ... })]` sets it.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use serde::Deserialize;

use crate::text::TextExpr;

/// The messages a type sets; the rest keep `clap_couture::Messages::DEFAULT`'s.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Messages {
    none_option: Option<TextExpr>,
    options_heading: Option<TextExpr>,
    usage_heading: Option<TextExpr>,
}

impl ToTokens for Messages {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let set_messages = [
            ("none_option", &self.none_option),
            ("options_heading", &self.options_heading),
            ("usage_heading", &self.usage_heading),
        ]
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
