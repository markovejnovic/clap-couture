//! The parts of `clap_derive` this macro has to agree with.
//!
//! clap does not expose its attribute model, so two facts are recomputed here with the rules
//! `clap_derive` applies: the subcommand name each variant gets, and which struct field holds the
//! subcommand. For
//!
//! ```text
//! struct Cli {
//!     #[command(subcommand)]
//!     cmd: Option<Cmd>,         // the subcommand type is `Cmd`
//! }
//!
//! #[command(rename_all = "snake_case")]
//! enum Cmd {
//!     DryRun,                   // named `dry_run`
//!     #[command(rename_all = "kebab-case")]
//!     FooBar,                   // named `foo-bar`
//!     #[command(name = "ls")]
//!     List,                     // named `ls`
//! }
//! ```
//!
//! Checked against `clap_derive` 4.6.7 (`src/item.rs`); re-check on a clap bump.

use heck::{
    ToKebabCase as _, ToLowerCamelCase as _, ToShoutySnakeCase as _, ToSnakeCase as _,
    ToUpperCamelCase as _,
};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::ToTokens;
use syn::{
    Attribute, Expr, ExprLit, ExprPath, GenericArgument, Lit, LitStr, PathArguments, PathSegment,
    Token, Type, TypePath, ext::IdentExt as _, meta::ParseNestedMeta, spanned::Spanned as _,
};

use crate::input::VariantInput;

/// A `rename_all` casing, mirroring `clap_derive`'s `CasingStyle`. The examples rename `DryRun`.
#[derive(Clone, Copy)]
enum CasingStyle {
    /// `dryRun`.
    Camel,
    /// `dry-run`, clap's default for subcommands.
    Kebab,
    /// `dryrun`.
    Lower,
    /// `DryRun`.
    Pascal,
    /// `DRY_RUN`.
    ScreamingSnake,
    /// `dry_run`.
    Snake,
    /// `DRYRUN`.
    Upper,
    /// `DryRun`, as written.
    Verbatim,
}

impl CasingStyle {
    fn apply(self, ident: &str) -> String {
        match self {
            Self::Camel => ident.to_lower_camel_case(),
            Self::Kebab => ident.to_kebab_case(),
            Self::Lower => ident.to_snake_case().replace('_', ""),
            Self::Pascal => ident.to_upper_camel_case(),
            Self::ScreamingSnake => ident.to_shouty_snake_case(),
            Self::Snake => ident.to_snake_case(),
            Self::Upper => ident.to_shouty_snake_case().replace('_', ""),
            Self::Verbatim => ident.to_owned(),
        }
    }

    /// The casing an item's `#[command(rename_all = "...")]` asks for, if any.
    fn from_attrs(attrs: &[Attribute]) -> Option<Self> {
        let mut casing = None;
        attrs.for_each_clap_key(|meta| {
            if meta.path.is_ident("rename_all") {
                let style = Self::from_lit(&meta.value()?.parse::<LitStr>()?.value());
                casing = style.or(casing);
            }
            Ok(())
        });
        casing
    }

    /// Parse a `rename_all` value the way clap does, ignoring case and separators: `"kebab-case"`,
    /// `"KebabCase"` and `"kebab"` are all [`Self::Kebab`]. `None` for a value clap rejects.
    fn from_lit(value: &str) -> Option<Self> {
        let normalized = value.to_upper_camel_case().to_lowercase();
        Some(match normalized.as_str() {
            "camel" | "camelcase" => Self::Camel,
            "kebab" | "kebabcase" => Self::Kebab,
            "pascal" | "pascalcase" => Self::Pascal,
            "screamingsnake" | "screamingsnakecase" => Self::ScreamingSnake,
            "snake" | "snakecase" => Self::Snake,
            "lower" | "lowercase" => Self::Lower,
            "upper" | "uppercase" => Self::Upper,
            "verbatim" | "verbatimcase" => Self::Verbatim,
            _ => return None,
        })
    }
}

/// The rule clap names one subcommand enum's variants by: the enum's `rename_all` casing, unless
/// a variant sets its own.
///
/// Under the default kebab-case, [`Self::name_of`] turns `DryRun` into `dry-run`, which is what
/// the user types and what `CATEGORIES` has to match.
pub(crate) struct SubcommandNaming {
    casing: CasingStyle,
}

impl SubcommandNaming {
    /// Read the enum's `#[command(rename_all = "...")]`, defaulting to kebab-case.
    pub(crate) fn from_container(attrs: &[Attribute]) -> Self {
        Self { casing: CasingStyle::from_attrs(attrs).unwrap_or(CasingStyle::Kebab) }
    }

    /// The name clap gives `variant`: `ls` for `#[command(name = "ls")] List`, otherwise the
    /// variant's or the enum's casing applied to the ident with any `r#` stripped (`r#Type` is
    /// `type`).
    pub(crate) fn name_of(&self, variant: &VariantInput) -> ClapName {
        variant.attrs.clap_name().unwrap_or_else(|| {
            let casing = CasingStyle::from_attrs(&variant.attrs).unwrap_or(self.casing);
            ClapName::Literal(casing.apply(&variant.ident.unraw().to_string()))
        })
    }
}

/// A subcommand's name or an arg's id, as clap gets it: a string literal, or any other expression,
/// like `name = LIST_NAME`, which the generated code evaluates as clap does.
#[derive(Clone)]
pub(crate) enum ClapName {
    Expr(Box<Expr>),
    Literal(String),
}

impl From<Expr> for ClapName {
    fn from(expr: Expr) -> Self {
        if let Expr::Lit(ExprLit { attrs: _, lit: Lit::Str(literal) }) = &expr {
            return Self::Literal(literal.value());
        }
        Self::Expr(Box::new(expr))
    }
}

impl ToTokens for ClapName {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            Self::Expr(expr) => expr.to_tokens(tokens),
            Self::Literal(name) => LitStr::new(name, Span::call_site()).to_tokens(tokens),
        }
    }
}

/// How clap treats a struct field.
#[derive(PartialEq, Eq)]
pub(crate) enum FieldKind {
    Arg,
    Flatten,
    Skip,
    Subcommand,
}

impl From<&[Attribute]> for FieldKind {
    fn from(attrs: &[Attribute]) -> Self {
        if attrs.has_clap_key("flatten") {
            Self::Flatten
        } else if attrs.has_clap_key("subcommand") {
            Self::Subcommand
        } else if attrs.has_clap_key("skip") {
            Self::Skip
        } else {
            Self::Arg
        }
    }
}

/// An `ArgAction` that takes no value, as `Count` in `#[arg(action = ArgAction::Count)]`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlagAction {
    Count,
    Help,
    HelpLong,
    HelpShort,
    SetFalse,
    SetTrue,
    Version,
}

impl FlagAction {
    /// The action `expr` names: a path ending in one of the variants, bare or after `ArgAction`.
    /// `None` for any other expression, which may take a value.
    fn from_expr(expr: &Expr) -> Option<Self> {
        let Expr::Path(ExprPath { attrs: _, qself: None, path }) = expr else { return None };
        let mut segments = path.segments.iter().rev();
        let variant = segments.next()?;
        if segments.next().is_some_and(|enum_segment| enum_segment.ident != "ArgAction") {
            return None;
        }
        Some(match variant.ident.to_string().as_str() {
            "Count" => Self::Count,
            "Help" => Self::Help,
            "HelpLong" => Self::HelpLong,
            "HelpShort" => Self::HelpShort,
            "SetFalse" => Self::SetFalse,
            "SetTrue" => Self::SetTrue,
            "Version" => Self::Version,
            _ => return None,
        })
    }
}

/// A container's, variant's or field's `#[command(...)]` / `#[arg(...)]` / `#[clap(...)]`
/// attributes, read the way clap reads them.
pub(crate) trait ClapAttrs {
    /// Where the first key that gives the arg a default is, if any: any key starting with
    /// `default_value` or `default_missing_value`, so the conditional `default_value_if(s)` too.
    fn clap_default_span(&self) -> Option<Span>;

    /// The `Count` in `#[arg(action = ArgAction::Count)]`, if the arg's action takes no value.
    fn clap_flag_action(&self) -> Option<FlagAction>;

    /// The `zone` in `#[arg(id = "zone")]`, or in the deprecated `#[arg(name = "zone")]`, if any.
    fn clap_id(&self) -> Option<ClapName>;

    /// The `ls` in `#[command(name = "ls")]`, if any.
    fn clap_name(&self) -> Option<ClapName>;

    /// Walk the keys of every `#[command(...)]` / `#[arg(...)]` / `#[clap(...)]` attribute,
    /// calling `on_key` on each: `name` then `about` for `#[command(name = "ls", about)]`. Any
    /// value `on_key` leaves unconsumed is skipped, so a visitor only needs to handle the keys it
    /// cares about.
    ///
    /// A parse error silently ends the walk of that attribute; `clap_derive` reports it.
    fn for_each_clap_key(&self, on_key: impl FnMut(&ParseNestedMeta<'_>) -> syn::Result<()>);

    /// Check for `key` with or without a value, as `subcommand` in `#[command(subcommand)]`.
    fn has_clap_key(&self, key: &str) -> bool;
}

impl ClapAttrs for [Attribute] {
    fn clap_default_span(&self) -> Option<Span> {
        let mut found = None;
        self.for_each_clap_key(|meta| {
            let gives_default = meta.path.get_ident().is_some_and(|key| {
                let key = key.to_string();
                key.starts_with("default_value") || key.starts_with("default_missing_value")
            });
            if gives_default {
                found.get_or_insert_with(|| meta.path.span());
            }
            Ok(())
        });
        found
    }

    fn clap_flag_action(&self) -> Option<FlagAction> {
        let mut found = None;
        self.for_each_clap_key(|meta| {
            if meta.path.is_ident("action") && meta.input.peek(Token![=]) {
                found = FlagAction::from_expr(&meta.value()?.parse::<Expr>()?);
            }
            Ok(())
        });
        found
    }

    fn clap_id(&self) -> Option<ClapName> {
        let mut found = None;
        self.for_each_clap_key(|meta| {
            if meta.path.is_ident("id") || meta.path.is_ident("name") {
                found = Some(meta.value()?.parse::<Expr>()?.into());
            }
            Ok(())
        });
        found
    }

    fn clap_name(&self) -> Option<ClapName> {
        let mut found = None;
        self.for_each_clap_key(|meta| {
            if meta.path.is_ident("name") {
                found.get_or_insert(meta.value()?.parse::<Expr>()?.into());
            }
            Ok(())
        });
        found
    }

    fn for_each_clap_key(&self, mut on_key: impl FnMut(&ParseNestedMeta<'_>) -> syn::Result<()>) {
        let is_clap = |attr: &&Attribute| {
            ["arg", "clap", "command"].iter().any(|name| attr.path().is_ident(name))
        };
        for attr in self.iter().filter(is_clap) {
            // Malformed attributes are clap_derive's to report.
            attr.parse_nested_meta(|meta| {
                on_key(&meta)?;
                meta.skip_value()
            })
            .ok();
        }
    }

    fn has_clap_key(&self, key: &str) -> bool {
        let mut found = false;
        self.for_each_clap_key(|meta| {
            found |= meta.path.is_ident(key);
            Ok(())
        });
        found
    }
}

trait ParseNestedMetaExt {
    /// Skip this key's value if nothing has read it yet.
    ///
    /// syn refuses to move on to the next key while the current one's value is unread, so after
    /// looking at a key we skip whatever is left: the `= "List"` of `about = "List"`, or the
    /// `(true)` of `next_line_help(true)`. A bare key like `subcommand` has no value to skip.
    fn skip_value(&self) -> syn::Result<()>;
}

impl ParseNestedMetaExt for ParseNestedMeta<'_> {
    fn skip_value(&self) -> syn::Result<()> {
        if self.input.peek(Token![=]) {
            self.value()?.parse::<syn::Expr>()?;
            return Ok(());
        }

        if self.input.peek(syn::token::Paren) {
            let parenthesized_value;
            syn::parenthesized!(parenthesized_value in self.input);
            parenthesized_value.parse::<TokenStream2>()?;
        }

        Ok(())
    }
}

/// A field's type, read the way clap reads it.
pub(crate) trait ClapFieldType {
    /// Check whether the field is a plain `bool`, which clap reads as a flag that takes no value.
    fn is_bool(&self) -> bool;

    /// Check whether clap reads the field as taking many values, which it infers from a `Vec`,
    /// bare or in an `Option`: `Option<Vec<String>>` is one.
    fn is_multi_value(&self) -> bool;

    /// Peel one `Option`, which clap reads as "may be absent": `Option<Cmd>` becomes `Cmd`, any
    /// other type comes back unchanged.
    fn peel_option(&self) -> &Self;

    /// The type clap parses each value as, for a field taking at most one: `Option<Option<T>>`,
    /// `Option<T>` and `T` all give `T`.
    fn value_type(&self) -> &Self;
}

/// `clap_derive`'s type matching (`src/utils/ty.rs`), ported as is: a type counts only as a
/// single-segment path, so `std::option::Option<T>` is a plain value to clap, not an `Option`.
trait ClapTypeShape {
    /// The path's one segment, seen through the invisible group a `macro_rules!` `$t:ty` wraps
    /// it in; `None` for a qualified, `::`-led or multi-segment path, or any other type.
    fn only_last_segment(&self) -> Option<&PathSegment>;

    /// The `T` of `name<T>`, with exactly one generic argument.
    fn subtype_if_named(&self, name: &str) -> Option<&Self>;
}

impl ClapFieldType for Type {
    fn is_bool(&self) -> bool {
        self.only_last_segment().is_some_and(|segment| {
            matches!(segment.arguments, PathArguments::None) && segment.ident == "bool"
        })
    }

    fn is_multi_value(&self) -> bool {
        self.subtype_if_named("Vec").is_some()
            || self.subtype_if_named("Option").is_some_and(|wrapped_type| {
                wrapped_type.subtype_if_named("Option").is_none()
                    && wrapped_type.subtype_if_named("Vec").is_some()
            })
    }

    fn peel_option(&self) -> &Self {
        self.subtype_if_named("Option").unwrap_or(self)
    }

    fn value_type(&self) -> &Self {
        self.peel_option().peel_option()
    }
}

impl ClapTypeShape for Type {
    fn only_last_segment(&self) -> Option<&PathSegment> {
        let mut ty = self;
        while let Self::Group(group) = ty {
            ty = &group.elem;
        }
        let Self::Path(TypePath { attrs: _, qself: None, path }) = ty else { return None };
        if path.leading_colon.is_some() {
            return None;
        }
        let mut segments = path.segments.iter();
        segments.next().filter(|_| segments.next().is_none())
    }

    fn subtype_if_named(&self, name: &str) -> Option<&Self> {
        let segment = self.only_last_segment().filter(|segment| segment.ident == name)?;
        let PathArguments::AngleBracketed(generic_arguments) = &segment.arguments else {
            return None;
        };
        let mut arguments = generic_arguments.args.iter();
        match (arguments.next(), arguments.next()) {
            (Some(GenericArgument::Type(wrapped_type)), None) => Some(wrapped_type),
            _ => None,
        }
    }
}
