//! `MESSAGES`: the text couture prints itself, as `#[couture(messages = { ... })]` sets it.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use serde::Deserialize;

use crate::text::TextExpr;

/// The messages a type sets; the rest keep `clap_couture::Messages::DEFAULT`'s.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Messages {
    editor_empty: Option<TextExpr>,
    editor_instructions: Option<TextExpr>,
    editor_instructions_optional: Option<TextExpr>,
    editor_returned_early: Option<TextExpr>,
    editor_wait_hint: Option<TextExpr>,
    editor_waiting: Option<TextExpr>,
    none: Option<TextExpr>,
    options: Option<TextExpr>,
    usage: Option<TextExpr>,
}

impl ToTokens for Messages {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let set_messages = [
            ("editor_empty", &self.editor_empty),
            ("editor_instructions", &self.editor_instructions),
            ("editor_instructions_optional", &self.editor_instructions_optional),
            ("editor_returned_early", &self.editor_returned_early),
            ("editor_wait_hint", &self.editor_wait_hint),
            ("editor_waiting", &self.editor_waiting),
            ("none", &self.none),
            ("options", &self.options),
            ("usage", &self.usage),
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
