//! The derive input as darling reads it.
//!
//! clap's own attributes (`#[command]`, `#[arg]`, `#[clap]`) are forwarded raw, as is the deriving
//! type's `#[couture(...)]`: the string keys of its `categories = { "key" = { ... } }` are not syn
//! `Meta`, so darling cannot parse it.

#![expect(
    clippy::unreachable,
    reason = "darling's derives expand to `unreachable!` for shapes `supports` already rejects"
)]

use darling::{
    FromDeriveInput, FromField, FromMeta, FromVariant, ast,
    util::{Flag, Override, SpannedValue},
};
use syn::{Attribute, Expr, Generics, Ident, LitStr, Type, ext::IdentExt as _};

use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, ClapName},
    text::TextExpr,
};

/// The item `#[derive(Couture)]` is on.
#[derive(FromDeriveInput)]
#[darling(supports(struct_any, enum_any), forward_attrs(clap, command, couture))]
pub(crate) struct CoutureInput {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) data: ast::Data<VariantInput, FieldInput>,
    pub(crate) generics: Generics,
    pub(crate) ident: Ident,
}

/// The options of `#[couture(editor(...))]`.
#[derive(Clone, Default, FromMeta)]
pub(crate) struct EditorOptions {
    /// The temp file's extension, without the dot; `txt` when left out.
    pub(crate) extension: Option<SpannedValue<String>>,
    /// The text the file starts with when the arg has no default.
    pub(crate) initial: Option<TextExpr>,
}

/// `#[couture(prompt)]`: bare, `= ...`, or `(dash, question = ...)`.
#[derive(Default)]
pub(crate) struct PromptOptions {
    /// `-` as the arg's value asks for it; nothing else does.
    pub(crate) dash: Flag,
    /// What it asks; `None` asks the arg's help text.
    pub(crate) question: Option<TextExpr>,
}

/// The `(...)` form of `#[couture(prompt)]`.
#[derive(FromMeta)]
struct PromptList {
    dash: Flag,
    question: Option<TextExpr>,
}

/// An attribute written bare (`Inherit`) or with `T` (`Explicit`), and where it was written.
pub(crate) type BareOr<T> = SpannedValue<Override<T>>;

/// What `#[couture(confirm)]` asks: `Inherit` when bare, the arg's help text; `Explicit` for
/// `= ...`.
pub(crate) type Question = BareOr<TextExpr>;

/// A field of a parser struct, an `Args` struct or a struct variant.
#[derive(FromField)]
#[darling(attributes(couture), forward_attrs(arg, clap, command))]
pub(crate) struct FieldInput {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) confirm: Option<Question>,
    /// `Inherit` for `#[couture(editor)]`, `Explicit` for `#[couture(editor(...))]`.
    pub(crate) editor: Option<BareOr<EditorOptions>>,
    pub(crate) ident: Option<Ident>,
    pub(crate) prompt: Option<SpannedValue<PromptOptions>>,
    /// `Inherit` for `#[couture(secret)]`, `Explicit` for `#[couture(secret(allow_argv))]`.
    pub(crate) secret: Option<BareOr<SecretOptions>>,
    /// `Inherit` for `#[couture(stdin)]`, `Explicit` for `#[couture(stdin(piped))]`.
    pub(crate) stdin: Option<BareOr<StdinOptions>>,
    pub(crate) ty: Type,
}

/// The options of `#[couture(secret(...))]`.
#[derive(FromMeta)]
pub(crate) struct SecretOptions {
    /// The value may come from the command line.
    pub(crate) allow_argv: Flag,
}

/// The options of `#[couture(stdin(...))]`.
#[derive(FromMeta)]
pub(crate) struct StdinOptions {
    /// Stdin is read whenever it is a pipe and the arg was left out, not only for `-`.
    pub(crate) piped: Flag,
}

/// A variant of a subcommand enum.
#[derive(FromVariant)]
#[darling(attributes(couture), forward_attrs(arg, clap, command))]
pub(crate) struct VariantInput {
    pub(crate) attrs: Vec<Attribute>,
    /// `#[couture(category = "...")]`: the label it files the variant under.
    pub(crate) category: Option<LitStr>,
    pub(crate) fields: ast::Fields<FieldInput>,
    pub(crate) ident: Ident,
}

impl FromMeta for PromptOptions {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(Self { dash: Flag::default(), question: Some(TextExpr::from_expr(expr)?) })
    }

    fn from_list(items: &[ast::NestedMeta]) -> darling::Result<Self> {
        let PromptList { dash, question } = PromptList::from_list(items)?;
        Ok(Self { dash, question })
    }

    fn from_word() -> darling::Result<Self> {
        Ok(Self::default())
    }
}

impl FieldInput {
    /// The id clap gives this field's arg: `#[arg(id = "...")]`, else the ident with any `r#`
    /// stripped (`r#type` is `type`). `None` for a tuple field.
    pub(crate) fn clap_arg_id(&self) -> Option<ClapName> {
        self.attrs.clap_assigned_name().or_else(|| {
            self.ident.as_ref().map(|ident| ClapName::Literal(ident.unraw().to_string()))
        })
    }

    /// Check whether this field's arg may take a value: not a `bool` flag, nor given an action
    /// that takes none. An action the derive can't read counts as taking one.
    pub(crate) fn takes_value(&self) -> bool {
        let bool_flag = self.ty.is_bool() && !self.attrs.has_clap_key("action");
        !bool_flag && self.attrs.clap_flag_action().is_none()
    }
}
