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
    editor_not_utf8: Option<TextExpr>,
    editor_returned_early: Option<TextExpr>,
    editor_waiting: Option<TextExpr>,
    finish_hint: Option<TextExpr>,
    none: Option<TextExpr>,
    options: Option<TextExpr>,
    selection_required: Option<TextExpr>,
    usage: Option<TextExpr>,
}

// Every field is spelled out, the unset ones from the defaults: a `..DEFAULT` struct update
// would trip clippy's `needless_update` in the user's crate whenever all of them are set.
impl ToTokens for Messages {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let messages = [
            ("editor_empty", &self.editor_empty),
            ("editor_instructions", &self.editor_instructions),
            ("editor_instructions_optional", &self.editor_instructions_optional),
            ("editor_not_utf8", &self.editor_not_utf8),
            ("editor_returned_early", &self.editor_returned_early),
            ("editor_waiting", &self.editor_waiting),
            ("finish_hint", &self.finish_hint),
            ("none", &self.none),
            ("options", &self.options),
            ("selection_required", &self.selection_required),
            ("usage", &self.usage),
        ]
        .map(|(name, text)| {
            let name = syn::Ident::new(name, proc_macro2::Span::call_site());
            text.as_ref().map_or_else(
                || quote!(#name: ::clap_couture::Messages::DEFAULT.#name),
                |text| quote!(#name: #text),
            )
        });
        tokens.extend(quote! { ::clap_couture::Messages { #(#messages,)* } });
    }
}
