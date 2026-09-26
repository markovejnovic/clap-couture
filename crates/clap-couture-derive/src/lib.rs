//! Derive macro for [`clap-couture`](https://docs.rs/clap-couture).

mod attrs;
mod clap_compat;
mod prompts;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, LitStr, parse_macro_input};

use crate::{
    attrs::{CategoriesSpec, Inherit},
    clap_compat::{DataStructExt as _, SubcommandNaming},
};

struct CategoryDef {
    description: Option<String>,
    label: String,
    title: Option<String>,
}

/// Which undeclared categories `#[category("...")]` may name.
enum InheritSpec {
    /// `inherit = true`: reference any category (skip the compile-time check).
    All,
    /// `inherit = ["a", ...]`: also accept these parent categories.
    List(Vec<String>),
    /// No `inherit` clause, or `inherit = false`.
    None,
}

/// Group a clap CLI's subcommands into categories in its `--help`.
///
/// On a subcommand enum, implements `clap_couture::Couture` from two attributes:
///
/// - `#[couture(categories = { "key" = { title = "...", description = "..." }, ... })]` on the enum
///   declares the categories in display order. `title` and `description` are optional.
/// - `#[category("key")]` on a variant files it under that category. Naming a category the enum
///   neither declares nor inherits is a compile error, unless it declares and inherits none.
///
/// `#[couture(inherit = ["key", ...])]` lets variants use categories declared on a parent
/// command, and `inherit = true` accepts any. A category not declared on the enum renders with
/// its key as the heading.
///
/// On a struct, takes the categories of its `#[command(subcommand)]` field, if any. Parse
/// through `clap_couture::CoutureParser` to get the grouped help.
///
/// With the `interactive` feature, `#[couture(prompt)]` or `#[couture(prompt = "...")]` on a field
/// asks for that arg when the user leaves it out; see `clap_couture::interactive`.
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
    let allowed = allowed_categories(&categories, inherit);

    let naming = SubcommandNaming::from_container(&input.attrs);

    let mut assignments: Vec<(String, String)> = Vec::new();
    for variant in &data.variants {
        let Some(category) = parse_category(&variant.attrs)? else {
            continue;
        };
        let label = category.value();
        if allowed.as_ref().is_some_and(|allowed| !allowed.contains(&label)) {
            return Err(syn::Error::new_spanned(
                &category,
                format!(
                    "category `{label}` is not declared or inherited in this enum's \
                     `#[couture(...)]`"
                ),
            ));
        }
        assignments.push((naming.name_of(variant), label));
    }

    // Heading order is first-appearance order in `CATEGORIES`, so emit by declared category.
    let mut pairs: Vec<TokenStream2> = Vec::new();
    for cat in &categories {
        let label = &cat.label;
        let title = option_str(cat.title.as_deref());
        let description = option_str(cat.description.as_deref());
        for pair in &assignments {
            let (name, assigned_label) = (&pair.0, &pair.1);
            if assigned_label == label {
                pairs.push(quote!((
                    ::clap_couture::CommandName::new(#name),
                    ::clap_couture::Category { description: #description, label: #label, title: #title }
                )));
            }
        }
    }
    // Categories not declared here carry only the label, which becomes their heading.
    for pair in &assignments {
        let (name, assigned_label) = (&pair.0, &pair.1);
        if !categories.iter().any(|c| &c.label == assigned_label) {
            pairs.push(quote!((
                ::clap_couture::CommandName::new(#name),
                ::clap_couture::Category {
                    description: ::core::option::Option::None,
                    label: #assigned_label,
                    title: ::core::option::Option::None,
                }
            )));
        }
    }

    let prompts = prompts::item(&prompts::enum_node(data, &naming)?);

    Ok(quote! {
        impl #impl_generics ::clap_couture::Couture for #ty #ty_generics #where_clause {
            const CATEGORIES: ::clap_couture::CommandCategoryMap =
                ::clap_couture::CommandCategoryMap::new(&[#(#pairs),*]);
            #prompts
        }
    })
}

/// Struct: delegate `CATEGORIES` to the `#[command(subcommand)]` field's type, if any, and emit
/// the `PROMPTS` tree.
fn expand_struct(input: &DeriveInput, data: &DataStruct) -> syn::Result<TokenStream2> {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let categories = data.clap_subcommand_type().map(|sub_ty| {
        quote! {
            const CATEGORIES: ::clap_couture::CommandCategoryMap =
                <#sub_ty as ::clap_couture::Couture>::CATEGORIES;
        }
    });

    let prompts = prompts::item(&prompts::fields_node(&data.fields)?);

    Ok(quote! {
        impl #impl_generics ::clap_couture::Couture for #ty #ty_generics #where_clause {
            #categories
            #prompts
        }
    })
}

/// The categories `#[category("...")]` may name, or `None` to accept any.
fn allowed_categories(categories: &[CategoryDef], inherit: InheritSpec) -> Option<Vec<String>> {
    let declared = categories.iter().map(|category| category.label.clone());
    match inherit {
        InheritSpec::All => None,
        // Declares and inherits nothing: nothing to check against.
        InheritSpec::None if categories.is_empty() => None,
        InheritSpec::None => Some(declared.collect()),
        InheritSpec::List(inherited) => Some(declared.chain(inherited).collect()),
    }
}

fn option_str(value: Option<&str>) -> TokenStream2 {
    value.map_or_else(
        || quote!(::core::option::Option::None),
        |value| quote!(::core::option::Option::Some(#value)),
    )
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
                description: category.description,
                label,
                title: category.title,
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
