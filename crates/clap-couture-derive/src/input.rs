//! The derive input as darling reads it.
//!
//! clap's own attributes (`#[command]`, `#[arg]`, `#[clap]`) and `#[category("...")]` are forwarded
//! raw, as is the enum's `#[couture(categories = { "key" = { ... } })]`: its string keys are not
//! syn `Meta`, so darling cannot parse it.

#![expect(
    clippy::unreachable,
    reason = "darling's derives expand to `unreachable!` for shapes `supports` already rejects"
)]

use darling::{
    FromDeriveInput, FromField, FromVariant, ast,
    util::{Override, SpannedValue},
};
use syn::{Attribute, Generics, Ident, Type, ext::IdentExt as _};

use crate::{clap_compat::ClapAttrs as _, text::TextExpr};

/// The item `#[derive(Couture)]` is on.
#[derive(FromDeriveInput)]
#[darling(supports(struct_any, enum_any), forward_attrs(clap, command, couture))]
pub(crate) struct CoutureInput {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) data: ast::Data<VariantInput, FieldInput>,
    pub(crate) generics: Generics,
    pub(crate) ident: Ident,
}

/// What `#[couture(prompt)]` or `#[couture(confirm)]` asks: `Inherit` when bare, the arg's help
/// text; `Explicit` for `= ...`.
pub(crate) type Question = SpannedValue<Override<TextExpr>>;

/// A field of a parser struct, an `Args` struct or a struct variant.
#[derive(FromField)]
#[darling(attributes(couture), forward_attrs(arg, clap, command))]
pub(crate) struct FieldInput {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) confirm: Option<Question>,
    pub(crate) ident: Option<Ident>,
    pub(crate) prompt: Option<Question>,
    pub(crate) ty: Type,
}

/// A variant of a subcommand enum.
#[derive(FromVariant)]
#[darling(forward_attrs(arg, category, clap, command))]
pub(crate) struct VariantInput {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) fields: ast::Fields<FieldInput>,
    pub(crate) ident: Ident,
}

impl FieldInput {
    /// The id clap gives this field's arg: `#[arg(id = "...")]`, else the ident with any `r#`
    /// stripped (`r#type` is `type`). `None` for a tuple field.
    pub(crate) fn clap_arg_id(&self) -> Option<String> {
        self.attrs.clap_id().or_else(|| self.ident.as_ref().map(|ident| ident.unraw().to_string()))
    }
}
