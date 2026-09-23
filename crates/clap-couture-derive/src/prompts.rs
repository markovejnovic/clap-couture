//! The `PROMPTS` tree: which args `#[couture(prompt)]` marks, and the command path to each.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Attribute, DataEnum, Field, Fields, LitStr, Token, spanned::Spanned as _};

use crate::{
    clap_compat::{ClapAttrsExt as _, FieldExt as _, SubcommandNaming, TypeExt as _},
    option_str,
};

/// How clap treats a struct field.
enum FieldKind {
    Arg,
    Flatten,
    Skip,
    Subcommand,
}

/// A `#[couture(prompt)]` or `#[couture(prompt = "...")]` on a field.
struct Prompt {
    question: Option<LitStr>,
    span: Span,
}

/// The `const PROMPTS` item for `node`, or nothing without the `interactive` feature.
pub(crate) fn item(node: &TokenStream2) -> TokenStream2 {
    if cfg!(feature = "interactive") {
        quote!(const PROMPTS: ::clap_couture::interactive::PromptNode = #node;)
    } else {
        TokenStream2::new()
    }
}

/// The node for a subcommand enum: one child per variant that can hold prompts.
pub(crate) fn enum_node(data: &DataEnum, naming: &SubcommandNaming) -> syn::Result<TokenStream2> {
    let mut children = Vec::new();
    for variant in &data.variants {
        let attrs = &variant.attrs;
        if attrs.has_clap_key("skip") || attrs.has_clap_key("external_subcommand") {
            continue;
        }
        let child = match &variant.fields {
            Fields::Named(_) => {
                let name = naming.name_of(variant);
                let node = fields_node(&variant.fields)?;
                quote!(::clap_couture::interactive::PromptChild::Subcommand(#name, &#node))
            }
            Fields::Unnamed(fields) => {
                let Some(field) = fields.unnamed.first() else { continue };
                let ty = &field.ty;
                let node = quote!(&<#ty as ::clap_couture::Couture>::PROMPTS);
                if attrs.has_clap_key("flatten") {
                    quote!(::clap_couture::interactive::PromptChild::Flatten(#node))
                } else {
                    let name = naming.name_of(variant);
                    quote!(::clap_couture::interactive::PromptChild::Subcommand(#name, #node))
                }
            }
            Fields::Unit => continue,
        };
        children.push(child);
    }
    Ok(quote! {
        ::clap_couture::interactive::PromptNode { args: &[], children: &[#(#children),*] }
    })
}

/// The node for one command level's fields: its marked args, plus its flattened and subcommand
/// types.
pub(crate) fn fields_node(fields: &Fields) -> syn::Result<TokenStream2> {
    let mut args = Vec::new();
    let mut children = Vec::new();
    for field in fields {
        let prompt = parse_prompt(&field.attrs)?;
        match (field_kind(&field.attrs), prompt) {
            (FieldKind::Arg, Some(prompt)) => args.push(spec(field, &prompt)?),
            (FieldKind::Flatten, None) => {
                let ty = &field.ty;
                children.push(quote! {
                    ::clap_couture::interactive::PromptChild::Flatten(
                        &<#ty as ::clap_couture::Couture>::PROMPTS
                    )
                });
            }
            (FieldKind::Subcommand, None) => {
                let ty = field.ty.peel_option();
                children.push(quote! {
                    ::clap_couture::interactive::PromptChild::Flatten(
                        &<#ty as ::clap_couture::Couture>::PROMPTS
                    )
                });
            }
            (FieldKind::Arg | FieldKind::Skip, None) => {}
            (FieldKind::Flatten, Some(prompt)) => {
                return Err(misplaced(&prompt, "the fields of the flattened type"));
            }
            (FieldKind::Skip, Some(prompt)) => {
                return Err(syn::Error::new(
                    prompt.span,
                    "`#[couture(prompt)]` cannot mark a field clap skips",
                ));
            }
            (FieldKind::Subcommand, Some(prompt)) => {
                return Err(misplaced(&prompt, "the fields of the subcommand's variants"));
            }
        }
    }
    Ok(quote! {
        ::clap_couture::interactive::PromptNode {
            args: &[#(#args),*],
            children: &[#(#children),*],
        }
    })
}

fn field_kind(attrs: &[Attribute]) -> FieldKind {
    if attrs.has_clap_key("flatten") {
        FieldKind::Flatten
    } else if attrs.has_clap_key("subcommand") {
        FieldKind::Subcommand
    } else if attrs.has_clap_key("skip") {
        FieldKind::Skip
    } else {
        FieldKind::Arg
    }
}

fn misplaced(prompt: &Prompt, instead: &str) -> syn::Error {
    syn::Error::new(
        prompt.span,
        format!("`#[couture(prompt)]` marks a single arg; put it on {instead} instead"),
    )
}

fn parse_prompt(attrs: &[Attribute]) -> syn::Result<Option<Prompt>> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("couture")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("prompt") {
                return Err(meta.error("expected `prompt` or `prompt = \"...\"`"));
            }
            let question = if meta.input.peek(Token![=]) {
                Some(meta.value()?.parse::<LitStr>()?)
            } else {
                None
            };
            found = Some(Prompt { question, span: meta.path.span() });
            Ok(())
        })?;
    }
    match found {
        Some(prompt) if !cfg!(feature = "interactive") => Err(syn::Error::new(
            prompt.span,
            "`#[couture(prompt)]` needs clap-couture's `interactive` feature; enable it or one of \
             `interactive-cliclack`, `interactive-dialoguer`, `interactive-inquire`",
        )),
        prompt => Ok(prompt),
    }
}

fn spec(field: &Field, prompt: &Prompt) -> syn::Result<TokenStream2> {
    if field.ty.is_vec() {
        return Err(syn::Error::new(
            prompt.span,
            "`#[couture(prompt)]` does not support multi-value args yet; take a single value or \
             drop the prompt",
        ));
    }
    let Some(id) = field.clap_arg_id() else {
        return Err(syn::Error::new(prompt.span, "`#[couture(prompt)]` needs a named field"));
    };
    let question = option_str(prompt.question.as_ref().map(LitStr::value).as_deref());
    Ok(quote!(::clap_couture::interactive::PromptSpec { id: #id, question: #question }))
}
