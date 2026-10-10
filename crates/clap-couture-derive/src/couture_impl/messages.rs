//! `MESSAGES`: the text couture prints itself, as `#[couture(messages = { ... })]` sets it.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use serde::Deserialize;
use syn::Attribute;

use crate::text::TextExpr;

/// The messages a type sets; the rest keep `clap_couture::Messages::DEFAULT`'s.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Messages {
    none_option: Option<TextExpr>,
    options_heading: Option<TextExpr>,
    usage_heading: Option<TextExpr>,
}

/// One `#[couture(...)]`'s `messages = { ... }`. Its other keys are read elsewhere.
#[derive(Deserialize)]
struct MessagesSpec {
    #[serde(default)]
    messages: Option<Messages>,
}

impl Messages {
    /// The messages of every `#[couture(messages = { ... })]` in `attrs`; for a message set more
    /// than once, the last one wins. `None` when none sets any.
    ///
    /// # Errors
    /// - A malformed `#[couture(...)]`, or a message couture doesn't print.
    pub(crate) fn parse(attrs: &[Attribute]) -> darling::Result<Option<Self>> {
        attrs.iter().filter(|attr| attr.path().is_ident("couture")).try_fold(
            None,
            |merged: Option<Self>, attr| {
                let list = attr.meta.require_list()?;
                let spec: MessagesSpec = serde_tokenstream::from_tokenstream_spanned(
                    list.delimiter.span(),
                    &list.tokens,
                )?;
                Ok(match (merged, spec.messages) {
                    (Some(earlier), Some(later)) => Some(later.or(earlier)),
                    (merged, later) => later.or(merged),
                })
            },
        )
    }

    /// These messages, with `earlier`'s filling the ones they don't set.
    fn or(self, earlier: Self) -> Self {
        Self {
            none_option: self.none_option.or(earlier.none_option),
            options_heading: self.options_heading.or(earlier.options_heading),
            usage_heading: self.usage_heading.or(earlier.usage_heading),
        }
    }
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
