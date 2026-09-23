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
use syn::meta::ParseNestedMeta;
use syn::{Attribute, DataStruct, LitStr, Token, Type, Variant, ext::IdentExt as _};

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
    pub(crate) fn name_of(&self, variant: &Variant) -> String {
        variant.attrs.clap_name().unwrap_or_else(|| {
            let casing = CasingStyle::from_attrs(&variant.attrs).unwrap_or(self.casing);
            casing.apply(&variant.ident.unraw().to_string())
        })
    }
}

/// Queries over an item's own `#[command(...)]` / `#[clap(...)]` attributes.
pub(crate) trait ClapAttrsExt {
    /// The `ls` in `#[command(name = "ls")]`, if any.
    fn clap_name(&self) -> Option<String>;

    /// Walk the keys of every `#[command(...)]` / `#[clap(...)]` attribute, calling `on_key` on
    /// each: `name` then `about` for `#[command(name = "ls", about)]`. Any value `on_key` leaves
    /// unconsumed is skipped, so a visitor only needs to handle the keys it cares about.
    ///
    /// A parse error silently ends the walk of that attribute, so `name = SOME_CONST` reads as
    /// no name.
    fn for_each_clap_key(&self, on_key: impl FnMut(&ParseNestedMeta<'_>) -> syn::Result<()>);

    /// Check for `#[command(subcommand)]` or `#[clap(subcommand)]`.
    fn is_clap_subcommand(&self) -> bool;
}

impl ClapAttrsExt for [Attribute] {
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
        let is_clap =
            |attr: &&Attribute| attr.path().is_ident("command") || attr.path().is_ident("clap");
        for attr in self.iter().filter(is_clap) {
            // Malformed attributes are clap_derive's to report.
            attr.parse_nested_meta(|meta| {
                on_key(&meta)?;
                meta.skip_value()
            })
            .ok();
        }
    }

    fn is_clap_subcommand(&self) -> bool {
        let mut found = false;
        self.for_each_clap_key(|meta| {
            if meta.path.is_ident("subcommand") {
                found = true;
            }
            Ok(())
        });
        found
    }
}

/// Finding the field clap treats as the subcommand.
pub(crate) trait DataStructExt {
    /// The type of the first `#[command(subcommand)]` / `#[clap(subcommand)]` field, with one
    /// `Option` peeled since clap allows optional subcommands: `Cmd` for `cmd: Option<Cmd>`.
    fn clap_subcommand_type(&self) -> Option<&Type>;
}

impl DataStructExt for DataStruct {
    fn clap_subcommand_type(&self) -> Option<&Type> {
        self.fields
            .iter()
            .find(|field| field.attrs.is_clap_subcommand())
            .map(|field| field.ty.peel_option())
    }
}

trait ParseNestedMetaExt {
    /// Consume the value of a key the visitor ignored, so `parse_nested_meta` can advance: the
    /// `= "List"` in `about = "List"`, or the `(true)` in `next_line_help(true)`. A no-op for bare
    /// flags like `subcommand` and for keys whose value the visitor already parsed.
    fn skip_value(&self) -> syn::Result<()>;
}

impl ParseNestedMetaExt for ParseNestedMeta<'_> {
    fn skip_value(&self) -> syn::Result<()> {
        if self.input.peek(Token![=]) {
            self.value()?.parse::<syn::Expr>()?;
            return Ok(());
        }
        if self.input.peek(syn::token::Paren) {
            let inner;
            syn::parenthesized!(inner in self.input);
            inner.parse::<TokenStream2>()?;
        }
        Ok(())
    }
}

trait TypeExt {
    /// Peel one `Option`: `Option<Cmd>` becomes `Cmd`, any other type comes back unchanged.
    fn peel_option(&self) -> &Self;
}

impl TypeExt for Type {
    fn peel_option(&self) -> &Self {
        let Self::Path(tp) = self else { return self };
        let Some(seg) = tp.path.segments.last() else { return self };
        if seg.ident != "Option" {
            return self;
        }
        let syn::PathArguments::AngleBracketed(args) = &seg.arguments else { return self };
        match args.args.first() {
            Some(syn::GenericArgument::Type(inner)) => inner,
            _ => self,
        }
    }
}
