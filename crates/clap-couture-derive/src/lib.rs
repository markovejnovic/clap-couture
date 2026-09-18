//! Derive macro for [`clap-couture`](https://docs.rs/clap-couture).
//!
//! `#[derive(Couture)]` behaves by shape, like clap's own derives:
//!
//! - On a **subcommand enum**, it reads two helper attributes and emits an `impl
//!   clap_couture::Couture` carrying a `CATEGORIES` const:
//!   - `#[couture(categories = { "key" = { title = "...", description = "..." }, ... }, inherit =
//!     [...])]` on the enum: category display order with optional metadata (`title` and
//!     `description` are each optional), plus an optional `inherit` clause (`true`, `false`, or
//!     `["key", ...]`).
//!   - `#[category("key")]` on a variant: assign it to a category. Must name a category declared or
//!     inherited by the enum.
//! - On a **parser struct**, it finds the `#[command(subcommand)]` field and emits inherent
//!   `couture_command` / `couture_parse` / `couture_try_parse` (and `*_from`) methods that build
//!   the grouped command from that field's categories.

use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Attribute, Data, DataEnum, DataStruct, DeriveInput, LitStr, Token, Type, Variant,
    ext::IdentExt, parse_macro_input,
};

mod attrs;

use crate::attrs::{CategoriesSpec, Inherit};

#[proc_macro_derive(Couture, attributes(category, couture))]
pub fn derive_couture(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    match &input.data {
        Data::Enum(data) => expand_enum(input, data),
        Data::Struct(data) => expand_struct(input, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "`Couture` can only be derived on a subcommand enum or a parser struct",
        )),
    }
}

/// Subcommand enum: emit the `CATEGORIES` const.
fn expand_enum(input: &DeriveInput, data: &DataEnum) -> syn::Result<TokenStream2> {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let (categories, inherit) = parse_categories(&input.attrs)?;
    // Categories that `#[category("...")]` may reference: those declared here,
    // plus any pulled in via `inherit = [...]`. `inherit = true` opts out of the
    // check; an enum that declares and inherits nothing is left permissive.
    let mut known: Vec<String> = categories.iter().map(|c| c.label.clone()).collect();
    let validate = match inherit {
        InheritSpec::All => false,
        InheritSpec::None => !known.is_empty(),
        InheritSpec::List(inherited) => {
            known.extend(inherited);
            true
        }
    };

    let casing = container_casing(&input.attrs);

    let mut assignments: Vec<(String, String)> = Vec::new();
    for variant in &data.variants {
        // Only variants with `#[category("...")]` contribute (and only then do we
        // resolve the clap name, which peeks into `#[command(...)]`).
        let Some(category) = parse_category(&variant.attrs)? else {
            continue;
        };
        let label = category.value();
        if validate && !known.contains(&label) {
            return Err(syn::Error::new_spanned(
                &category,
                format!(
                    "category `{label}` is not declared or inherited in this enum's \
                     `#[couture(...)]`"
                ),
            ));
        }
        assignments.push((resolve_name(variant, casing), label));
    }

    // Emit `(CommandName, Category)` pairs grouped by declared-category order, so
    // headings display in the `#[couture(categories = { ... })]` order (commands
    // within a category stay in variant order).
    let mut pairs: Vec<TokenStream2> = Vec::new();
    for cat in &categories {
        let label = &cat.label;
        let title = option_str(cat.title.as_deref());
        let description = option_str(cat.description.as_deref());
        for (name, assigned_label) in &assignments {
            if assigned_label == label {
                pairs.push(quote!((
                    ::clap_couture::CommandName::new(#name),
                    ::clap_couture::Category { label: #label, title: #title, description: #description }
                )));
            }
        }
    }
    // Assignments to categories not declared here (inherited / permissive) carry
    // just the label; the command that owns the category supplies title/description.
    for (name, assigned_label) in &assignments {
        if !categories.iter().any(|c| &c.label == assigned_label) {
            pairs.push(quote!((
                ::clap_couture::CommandName::new(#name),
                ::clap_couture::Category {
                    label: #assigned_label,
                    title: ::core::option::Option::None,
                    description: ::core::option::Option::None,
                }
            )));
        }
    }

    Ok(quote! {
        impl #impl_generics ::clap_couture::Couture for #ty #ty_generics #where_clause {
            const CATEGORIES: ::clap_couture::CommandCategoryMap =
                ::clap_couture::CommandCategoryMap::new(&[#(#pairs),*]);
        }
    })
}

/// Parser struct: emit inherent `couture_*` methods driven by the type's
/// `#[command(subcommand)]` field.
fn expand_struct(input: &DeriveInput, data: &DataStruct) -> syn::Result<TokenStream2> {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let sub_ty = find_subcommand_type(data).ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            "`#[derive(Couture)]` on a struct requires a `#[command(subcommand)]` field",
        )
    })?;
    let sub_ty = unwrap_option(sub_ty);

    Ok(quote! {
        impl #impl_generics #ty #ty_generics #where_clause {
            /// The clap [`Command`](::clap::Command) with couture's grouped help installed.
            #[must_use]
            pub fn couture_command() -> ::clap::Command {
                ::clap_couture::CommandExt::with_couture::<#sub_ty>(
                    <Self as ::clap::CommandFactory>::command(),
                )
            }

            /// Parse from `std::env::args_os()`, exiting on error (like `clap::Parser::parse`).
            #[must_use]
            pub fn couture_parse() -> Self {
                let mut cmd = Self::couture_command();
                let mut matches = cmd.get_matches_mut();
                match <Self as ::clap::FromArgMatches>::from_arg_matches_mut(&mut matches) {
                    ::core::result::Result::Ok(v) => v,
                    ::core::result::Result::Err(e) => e.format(&mut cmd).exit(),
                }
            }

            /// Fallible [`couture_parse`](Self::couture_parse).
            pub fn couture_try_parse() -> ::core::result::Result<Self, ::clap::Error> {
                let mut matches = Self::couture_command().try_get_matches()?;
                <Self as ::clap::FromArgMatches>::from_arg_matches_mut(&mut matches)
                    .map_err(|e| e.format(&mut Self::couture_command()))
            }

            /// Parse from an explicit argument iterator, exiting on error.
            #[must_use]
            pub fn couture_parse_from<I, T>(itr: I) -> Self
            where
                I: ::core::iter::IntoIterator<Item = T>,
                T: ::core::convert::Into<::std::ffi::OsString> + ::core::clone::Clone,
            {
                let mut cmd = Self::couture_command();
                let mut matches = match cmd.try_get_matches_from_mut(itr) {
                    ::core::result::Result::Ok(m) => m,
                    ::core::result::Result::Err(e) => e.exit(),
                };
                match <Self as ::clap::FromArgMatches>::from_arg_matches_mut(&mut matches) {
                    ::core::result::Result::Ok(v) => v,
                    ::core::result::Result::Err(e) => e.format(&mut cmd).exit(),
                }
            }

            /// Fallible [`couture_parse_from`](Self::couture_parse_from).
            pub fn couture_try_parse_from<I, T>(itr: I) -> ::core::result::Result<Self, ::clap::Error>
            where
                I: ::core::iter::IntoIterator<Item = T>,
                T: ::core::convert::Into<::std::ffi::OsString> + ::core::clone::Clone,
            {
                let mut cmd = Self::couture_command();
                let mut matches = cmd.try_get_matches_from_mut(itr)?;
                <Self as ::clap::FromArgMatches>::from_arg_matches_mut(&mut matches)
                    .map_err(|e| e.format(&mut cmd))
            }
        }
    })
}

/// Type of the first `#[command(subcommand)]` / `#[clap(subcommand)]` field.
fn find_subcommand_type(data: &DataStruct) -> Option<&Type> {
    data.fields.iter().find(|field| is_subcommand(&field.attrs)).map(|field| &field.ty)
}

/// Peel a single `Option<T>` wrapper (clap allows optional subcommands).
fn unwrap_option(ty: &Type) -> &Type {
    let Type::Path(tp) = ty else { return ty };
    let Some(seg) = tp.path.segments.last() else { return ty };
    if seg.ident != "Option" {
        return ty;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else { return ty };
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner)) => inner,
        _ => ty,
    }
}

fn option_str(value: Option<&str>) -> TokenStream2 {
    match value {
        Some(value) => quote!(::core::option::Option::Some(#value)),
        None => quote!(::core::option::Option::None),
    }
}

struct CategoryDef {
    label: String,
    title: Option<String>,
    description: Option<String>,
}

/// How an enum relates to categories declared elsewhere (e.g. its parent
/// command). Purely a compile-time construct — it never reaches the `CATEGORIES` const.
enum InheritSpec {
    /// No `inherit` clause.
    None,
    /// `inherit = true`: reference any category (skip the compile-time check).
    All,
    /// `inherit = ["a", ...]`: also accept these parent categories.
    List(Vec<String>),
}

fn parse_categories(attrs: &[Attribute]) -> syn::Result<(Vec<CategoryDef>, InheritSpec)> {
    let mut out = Vec::new();
    let mut inherit = InheritSpec::None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("couture")) {
        let list = attr.meta.require_list()?;
        let spec: CategoriesSpec =
            serde_tokenstream::from_tokenstream_spanned(list.delimiter.span(), &list.tokens)?;
        for (label, category) in spec.categories {
            out.push(CategoryDef {
                label,
                title: category.title,
                description: category.description,
            });
        }
        inherit = match spec.inherit {
            None | Some(Inherit::All(false)) => InheritSpec::None,
            Some(Inherit::All(true)) => InheritSpec::All,
            Some(Inherit::List(list)) => InheritSpec::List(list),
        };
    }
    Ok((out, inherit))
}

fn parse_category(attrs: &[Attribute]) -> syn::Result<Option<LitStr>> {
    let mut found = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("category")) {
        found = Some(attr.parse_args::<LitStr>()?);
    }
    Ok(found)
}

/// Resolve a variant's clap subcommand name: an explicit `name = "..."` if
/// present, otherwise the enum's `rename_all` casing (default kebab-case) applied
/// to the raw-stripped variant ident — mirroring clap's own naming.
fn resolve_name(variant: &Variant, casing: CasingStyle) -> String {
    explicit_name(&variant.attrs)
        .unwrap_or_else(|| casing.apply(&variant.ident.unraw().to_string()))
}

/// The container-level `#[command(rename_all = "...")]`, defaulting to clap's
/// kebab-case for subcommands.
fn container_casing(attrs: &[Attribute]) -> CasingStyle {
    let mut casing = CasingStyle::Kebab;
    scan_clap_meta(attrs, |meta| {
        if meta.path.is_ident("rename_all")
            && let Some(style) = CasingStyle::from_lit(&meta.value()?.parse::<LitStr>()?.value())
        {
            casing = style;
        }
        Ok(())
    });
    casing
}

/// The explicit `#[command(name = "...")]`, if any.
fn explicit_name(attrs: &[Attribute]) -> Option<String> {
    let mut found = None;
    scan_clap_meta(attrs, |meta| {
        if meta.path.is_ident("name") {
            found.get_or_insert(meta.value()?.parse::<LitStr>()?.value());
        }
        Ok(())
    });
    found
}

/// Whether a field carries `#[command(subcommand)]` / `#[clap(subcommand)]`.
fn is_subcommand(attrs: &[Attribute]) -> bool {
    let mut found = false;
    scan_clap_meta(attrs, |meta| {
        if meta.path.is_ident("subcommand") {
            found = true;
        }
        Ok(())
    });
    found
}

/// Walk the keys of every `#[command(...)]` / `#[clap(...)]` attribute, calling
/// `on_key` for each. Any value `on_key` leaves unconsumed is skipped, so a
/// visitor only needs to handle the keys it cares about.
fn scan_clap_meta(
    attrs: &[Attribute],
    mut on_key: impl FnMut(&syn::meta::ParseNestedMeta<'_>) -> syn::Result<()>,
) {
    for attr in attrs.iter().filter(|a| a.path().is_ident("command") || a.path().is_ident("clap")) {
        let _ = attr.parse_nested_meta(|meta| {
            on_key(&meta)?;
            consume_meta_value(&meta)
        });
    }
}

/// Consume the value of a meta key we don't care about (`= value` or `(...)`),
/// so `parse_nested_meta` can advance. A no-op for bare flags and keys whose
/// value a visitor already parsed.
fn consume_meta_value(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<syn::Expr>()?;
    } else if meta.input.peek(syn::token::Paren) {
        let inner;
        syn::parenthesized!(inner in meta.input);
        inner.parse::<TokenStream2>()?;
    }
    Ok(())
}

/// clap's subcommand-name casing styles, matching `clap_derive`'s `CasingStyle`.
#[derive(Clone, Copy)]
enum CasingStyle {
    Camel,
    Kebab,
    Pascal,
    ScreamingSnake,
    Snake,
    Lower,
    Upper,
    Verbatim,
}

impl CasingStyle {
    /// Parse a `rename_all` value the way clap does (case/separator-insensitive).
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

    fn apply(self, ident: &str) -> String {
        match self {
            Self::Camel => ident.to_lower_camel_case(),
            Self::Kebab => ident.to_kebab_case(),
            Self::Pascal => ident.to_upper_camel_case(),
            Self::ScreamingSnake => ident.to_shouty_snake_case(),
            Self::Snake => ident.to_snake_case(),
            Self::Lower => ident.to_snake_case().replace('_', ""),
            Self::Upper => ident.to_shouty_snake_case().replace('_', ""),
            Self::Verbatim => ident.to_string(),
        }
    }
}
