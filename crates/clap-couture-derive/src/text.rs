//! User-facing text the attributes take, written into the generated code as `clap_couture::Text`.

use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote, quote_spanned};
use syn::{Expr, ExprLit, Lit, spanned::Spanned as _};

use crate::runtime::Runtime;

/// The expression of a `title = ...`, `description = ...` or `prompt = ...`. A string literal is
/// kept as is; any other expression is evaluated each time the text is shown, like clap's
/// `about = expr`.
#[derive(Clone)]
pub(crate) struct TextExpr(Expr);

/// An `Option<clap_couture::Text>` in the generated code: `Some(...)` or `None`.
pub(crate) struct OptionalText<'text>(Option<&'text TextExpr>);

impl TextExpr {
    /// Each `{id}` a string literal names, as a confirmation fills them in; none for any other
    /// expression, whose text is only known when shown.
    pub(crate) fn named_ids(&self) -> Vec<String> {
        let Expr::Lit(ExprLit { attrs: _, lit: Lit::Str(literal) }) = &self.0 else {
            return Vec::new();
        };
        let text = literal.value();
        let mut ids = Vec::new();
        let mut rest = text.as_str();
        while let Some(brace) = rest.find(['{', '}']) {
            let (_, tail) = rest.split_at(brace);
            if let Some(after) = tail.strip_prefix("{{").or_else(|| tail.strip_prefix("}}")) {
                rest = after;
                continue;
            }
            // Past a `{id}` naming no arg, the runtime goes on inside it, so every candidate
            // counts.
            if let Some((id, _)) = tail.strip_prefix('{').and_then(|inner| inner.split_once('}')) {
                ids.push(id.to_owned());
            }
            (_, rest) = tail.split_at(1);
        }
        ids
    }

    /// Where the expression was written.
    pub(crate) fn span(&self) -> proc_macro2::Span {
        self.0.span()
    }
}

impl From<Expr> for TextExpr {
    fn from(expr: Expr) -> Self {
        Self(expr)
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
            quote!(#Runtime::Text::Literal(#literal))
        } else {
            quote_spanned! {expr.span()=>
                #Runtime::Text::Computed(|| ::core::convert::Into::into(#expr))
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
