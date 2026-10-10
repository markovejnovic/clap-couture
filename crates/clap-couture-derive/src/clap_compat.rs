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
use proc_macro2::TokenStream as TokenStream2;
use syn::{Attribute, LitStr, Token, Type, ext::IdentExt as _, meta::ParseNestedMeta};

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
    pub(crate) fn name_of(&self, variant: &VariantInput) -> String {
        variant.attrs.clap_name().unwrap_or_else(|| {
            let casing = CasingStyle::from_attrs(&variant.attrs).unwrap_or(self.casing);
            casing.apply(&variant.ident.unraw().to_string())
        })
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

/// A container's, variant's or field's `#[command(...)]` / `#[arg(...)]` / `#[clap(...)]`
/// attributes, read the way clap reads them.
pub(crate) trait ClapAttrs {
    /// The `zone` in `#[arg(id = "zone")]`, or in the deprecated `#[arg(name = "zone")]`, if any.
    fn clap_id(&self) -> Option<String>;

    /// The `ls` in `#[command(name = "ls")]`, if any.
    fn clap_name(&self) -> Option<String>;

    /// Walk the keys of every `#[command(...)]` / `#[arg(...)]` / `#[clap(...)]` attribute,
    /// calling `on_key` on each: `name` then `about` for `#[command(name = "ls", about)]`. Any
    /// value `on_key` leaves unconsumed is skipped, so a visitor only needs to handle the keys it
    /// cares about.
    ///
    /// A parse error silently ends the walk of that attribute, so `name = SOME_CONST` reads as
    /// no name.
    fn for_each_clap_key(&self, on_key: impl FnMut(&ParseNestedMeta<'_>) -> syn::Result<()>);

    /// Check for any key that gives the arg a default: `default_value`, `default_value_t`,
    /// `default_values_t`, `default_missing_value`, and their `_os` forms.
    fn has_clap_default(&self) -> bool;

    /// Check for `key` with or without a value, as `subcommand` in `#[command(subcommand)]`.
    fn has_clap_key(&self, key: &str) -> bool;
}

impl ClapAttrs for [Attribute] {
    fn clap_id(&self) -> Option<String> {
        let mut found = None;
        self.for_each_clap_key(|meta| {
            if meta.path.is_ident("id") || meta.path.is_ident("name") {
                found = Some(meta.value()?.parse::<LitStr>()?.value());
            }
            Ok(())
        });
        found
    }

    fn clap_name(&self) -> Option<String> {
        let mut found = None;
        self.for_each_clap_key(|meta| {
            if meta.path.is_ident("name") {
                found.get_or_insert(meta.value()?.parse::<LitStr>()?.value());
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

    fn has_clap_default(&self) -> bool {
        let mut found = false;
        self.for_each_clap_key(|meta| {
            found |= meta.path.get_ident().is_some_and(|key| {
                let key = key.to_string();
                key.starts_with("default_value") || key.starts_with("default_missing_value")
            });
            Ok(())
        });
        found
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
}

impl ClapFieldType for Type {
    fn is_bool(&self) -> bool {
        // As clap's derive decides it: a bare `bool`, not `std::primitive::bool`.
        matches!(self, Self::Path(type_path) if type_path.qself.is_none() && type_path.path.is_ident("bool"))
    }

    fn is_multi_value(&self) -> bool {
        let Self::Path(type_path) = self.peel_option() else { return false };
        type_path.path.segments.last().is_some_and(|last_segment| last_segment.ident == "Vec")
    }

    fn peel_option(&self) -> &Self {
        let Self::Path(type_path) = self else { return self };
        let Some(last_segment) = type_path.path.segments.last() else { return self };
        if last_segment.ident != "Option" {
            return self;
        }
        let syn::PathArguments::AngleBracketed(generic_arguments) = &last_segment.arguments else {
            return self;
        };
        match generic_arguments.args.first() {
            Some(syn::GenericArgument::Type(wrapped_type)) => wrapped_type,
            _ => self,
        }
    }
}
