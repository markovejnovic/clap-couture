//! Literals written into the generated code.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

/// An `Option<&'static str>` literal in the generated code: `Some("...")` or `None`.
pub(crate) struct OptionStrLiteral<'value>(Option<&'value str>);

impl<'value> From<Option<&'value str>> for OptionStrLiteral<'value> {
    fn from(value: Option<&'value str>) -> Self {
        Self(value)
    }
}

impl<'value> From<&'value Option<String>> for OptionStrLiteral<'value> {
    fn from(value: &'value Option<String>) -> Self {
        Self(value.as_deref())
    }
}

impl ToTokens for OptionStrLiteral<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(self.0.map_or_else(
            || quote!(::core::option::Option::None),
            |value| quote!(::core::option::Option::Some(#value)),
        ));
    }
}
