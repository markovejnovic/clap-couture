//! User-facing text the attributes take, written into the generated code as `clap_couture::Text`.

use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote, quote_spanned};
use serde::{Deserialize, Deserializer};
use serde_tokenstream::ParseWrapper;
use syn::{Expr, ExprLit, Lit, spanned::Spanned as _};

/// The expression of a `title = ...`, `description = ...` or `prompt = ...`. A string literal is
/// kept as is; any other expression is evaluated each time the text is shown, like clap's
/// `about = expr`.
#[derive(Clone)]
pub(crate) struct TextExpr(Expr);

/// An `Option<clap_couture::Text>` in the generated code: `Some(...)` or `None`.
pub(crate) struct OptionalText<'text>(Option<&'text TextExpr>);

impl<'de> Deserialize<'de> for TextExpr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ParseWrapper::<Expr>::deserialize(deserializer).map(|expr| Self(expr.into_inner()))
    }
}

// darling's own `syn::Expr` parses a string literal's contents as an expression; text keeps the
// literal.
impl FromMeta for TextExpr {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(Self(expr.clone()))
    }
}

impl ToTokens for TextExpr {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let expr = &self.0;
        tokens.extend(if let Expr::Lit(ExprLit { attrs: _, lit: Lit::Str(literal) }) = expr {
            quote!(::clap_couture::Text::Literal(#literal))
        } else {
            quote_spanned! {expr.span()=>
                ::clap_couture::Text::Computed(|| ::core::convert::Into::into(#expr))
            }
        });
    }
}

impl<'text> From<&'text Option<TextExpr>> for OptionalText<'text> {
    fn from(text: &'text Option<TextExpr>) -> Self {
        Self(text.as_ref())
    }
}

impl ToTokens for OptionalText<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(self.0.map_or_else(
            || quote!(::core::option::Option::None),
            |text| quote!(::core::option::Option::Some(#text)),
        ));
    }
}
