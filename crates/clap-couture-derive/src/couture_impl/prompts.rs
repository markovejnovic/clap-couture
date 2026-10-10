//! `PROMPTS`: the args `#[couture(prompt)]` marks, and the command path to each.

use darling::{
    Error,
    ast::Style,
    util::{Override, SpannedValue},
};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::{Attribute, Type};

use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, SubcommandNaming},
    input::{FieldInput, VariantInput},
    literals::OptionStrLiteral,
};

/// The prompted args of one command level, and the tree below it.
pub(crate) struct PromptNode {
    args: Vec<PromptSpec>,
    children: Vec<PromptChild>,
}

/// What hangs off one command level besides its own prompted args.
enum PromptChild {
    /// A type merged into this level: a flattened `Args` type, or the subcommand enum.
    Flatten(Type),
    /// A struct variant: a subcommand whose args are its own fields.
    Inline(String, PromptNode),
    /// A tuple variant: a subcommand whose args are another type's.
    Subcommand(String, Type),
}

/// One `#[couture(prompt)]`.
struct PromptSpec {
    /// The clap id of the prompted arg.
    id: String,
    /// The `prompt = "..."` override; `None` asks the arg's help text.
    question: Option<String>,
}

/// How clap treats a struct field.
enum FieldKind {
    Arg,
    Flatten,
    Skip,
    Subcommand,
}

impl PromptNode {
    /// The node for one command level's fields: its prompted args, plus its flattened and
    /// subcommand types.
    ///
    /// # Errors
    /// - A `#[couture(prompt)]` without the `interactive` feature.
    /// - A `#[couture(prompt)]` on a field that is not a single named arg.
    pub(crate) fn of_fields(fields: &[FieldInput]) -> darling::Result<Self> {
        let mut errors = Error::accumulator();
        let args = fields
            .iter()
            .filter_map(|field| errors.handle(PromptSpec::of_field(field)).flatten())
            .collect();
        let children = fields.iter().filter_map(PromptChild::of_field).collect();
        errors.finish_with(Self { args, children })
    }

    /// The node for a subcommand enum: one child per variant that can hold prompts.
    ///
    /// # Errors
    /// As [`Self::of_fields`], for a struct variant's fields.
    pub(crate) fn of_variants(
        variants: &[VariantInput],
        naming: &SubcommandNaming,
    ) -> darling::Result<Self> {
        let mut errors = Error::accumulator();
        let children = variants
            .iter()
            .filter_map(|variant| errors.handle(PromptChild::of_variant(variant, naming)).flatten())
            .collect();
        errors.finish_with(Self { args: Vec::new(), children })
    }
}

impl PromptChild {
    /// A flattened or subcommand field's type, merged into its level.
    fn of_field(field: &FieldInput) -> Option<Self> {
        matches!(
            FieldKind::from(field.attrs.as_slice()),
            FieldKind::Flatten | FieldKind::Subcommand
        )
        .then(|| Self::Flatten(field.ty.peel_option().clone()))
    }

    /// The child a variant hangs off its enum's node, if it can hold prompts.
    fn of_variant(
        variant: &VariantInput,
        naming: &SubcommandNaming,
    ) -> darling::Result<Option<Self>> {
        let attrs = variant.attrs.as_slice();
        if attrs.has_clap_key("skip") || attrs.has_clap_key("external_subcommand") {
            return Ok(None);
        }
        match variant.fields.style {
            Style::Struct => PromptNode::of_fields(&variant.fields.fields)
                .map(|node| Some(Self::Inline(naming.name_of(variant), node))),
            Style::Tuple => Ok(variant.fields.fields.first().map(|field| {
                if attrs.has_clap_key("flatten") {
                    Self::Flatten(field.ty.clone())
                } else {
                    Self::Subcommand(naming.name_of(variant), field.ty.clone())
                }
            })),
            Style::Unit => Ok(None),
        }
    }
}

impl PromptSpec {
    fn new(field: &FieldInput, prompt: &SpannedValue<Override<String>>) -> darling::Result<Self> {
        let span = prompt.span();
        if !cfg!(feature = "interactive") {
            return Err(Error::custom(
                "`#[couture(prompt)]` needs clap-couture's `interactive` feature; enable it or \
                 one of `interactive-cliclack`, `interactive-dialoguer`, `interactive-inquire`",
            )
            .with_span(&span));
        }
        if field.ty.is_multi_value() {
            return Err(Error::custom(
                "`#[couture(prompt)]` does not support multi-value args yet; take a single value \
                 or drop the prompt",
            )
            .with_span(&span));
        }
        let Some(id) = field.clap_arg_id() else {
            return Err(Error::custom("`#[couture(prompt)]` needs a named field").with_span(&span));
        };
        let question = match &**prompt {
            Override::Inherit => None,
            Override::Explicit(question) => Some(question.clone()),
        };
        Ok(Self { id, question })
    }

    /// The field's `#[couture(prompt)]`, if it has one.
    fn of_field(field: &FieldInput) -> darling::Result<Option<Self>> {
        field
            .prompt
            .as_ref()
            .map(|prompt| match FieldKind::from(field.attrs.as_slice()) {
                FieldKind::Arg => Self::new(field, prompt),
                FieldKind::Flatten => {
                    Err(misplaced(prompt.span(), "the fields of the flattened type"))
                }
                FieldKind::Skip => {
                    Err(Error::custom("`#[couture(prompt)]` cannot mark a field clap skips")
                        .with_span(&prompt.span()))
                }
                FieldKind::Subcommand => {
                    Err(misplaced(prompt.span(), "the fields of the subcommand's variants"))
                }
            })
            .transpose()
    }
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

impl ToTokens for PromptNode {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let (args, children) = (&self.args, &self.children);
        tokens.extend(quote! {
            ::clap_couture::interactive::PromptNode {
                args: &[#(#args),*],
                children: &[#(#children),*],
            }
        });
    }
}

impl ToTokens for PromptChild {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(match self {
            Self::Flatten(ty) => {
                let node = probe(ty);
                quote!(::clap_couture::interactive::PromptChild::Flatten(#node))
            }
            Self::Inline(name, node) => {
                quote!(::clap_couture::interactive::PromptChild::Subcommand(#name, &#node))
            }
            Self::Subcommand(name, ty) => {
                let node = probe(ty);
                quote!(::clap_couture::interactive::PromptChild::Subcommand(#name, #node))
            }
        });
    }
}

impl ToTokens for PromptSpec {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let id = &self.id;
        let question = OptionStrLiteral::from(&self.question);
        tokens.extend(
            quote!(::clap_couture::interactive::PromptSpec { id: #id, question: #question }),
        );
    }
}

fn misplaced(span: Span, instead: &str) -> Error {
    Error::custom(format!("`#[couture(prompt)]` marks a single arg; put it on {instead} instead"))
        .with_span(&span)
}

/// `ty`'s prompted args, or none when `ty` does not implement `Couture`.
fn probe(ty: &Type) -> TokenStream2 {
    quote!(<::clap_couture::interactive::Probe<#ty>>::PROMPTS)
}
