//! `PROMPTED_ARGS`: the args `#[couture(prompt)]` and `#[couture(confirm)]` mark, and the command
//! path to each.

use darling::{Error, ast::Style, util::Override};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::Type;

use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, FieldKind, SubcommandNaming},
    input::{FieldInput, Question, VariantInput},
    text::{OptionalText, TextExpr},
};

/// The prompted args of one command level, and the tree below it.
pub(crate) struct PromptedArgTree {
    /// Merged trees and subcommands.
    branches: Vec<PromptedArgBranch>,
    /// The args prompted for at this level.
    own_args: Vec<PromptAttr>,
}

/// What hangs off one command level besides its own prompted args.
enum PromptedArgBranch {
    /// A type merged into this level: a flattened `Args` type, or the subcommand enum.
    Flattened(Type),
    /// A struct variant: a subcommand whose args are its own fields.
    Inline(String, PromptedArgTree),
    /// A tuple variant: a subcommand whose args are another type's.
    Subcommand(String, Type),
}

/// One `#[couture(prompt)]` or `#[couture(confirm)]`, with or without `= "..."`.
struct PromptAttr {
    /// The clap id of the arg it is on.
    arg_id: String,
    /// What the answer is for.
    purpose: Purpose,
    /// The `= ...` override; `None` asks the arg's help text.
    question: Option<TextExpr>,
}

/// What a prompt's answer is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Confirmation,
    Value,
}

impl PromptedArgTree {
    /// The tree for one command level's fields: its prompted args, plus its flattened and
    /// subcommand types.
    ///
    /// # Errors
    /// - A `#[couture(prompt)]` or `#[couture(confirm)]` without the `interactive` feature.
    /// - Either on a field that is not a single named arg, or both on one field.
    /// - A `#[couture(confirm)]` on a field that is not a `bool`.
    pub(crate) fn of_fields(fields: &[FieldInput]) -> darling::Result<Self> {
        let mut errors = Error::accumulator();
        let own_args = fields
            .iter()
            .filter_map(|field| errors.handle(PromptAttr::of_field(field)).flatten())
            .collect();
        let branches = fields.iter().filter_map(PromptedArgBranch::of_field).collect();
        errors.finish_with(Self { branches, own_args })
    }

    /// The tree for a subcommand enum: one branch per variant that can hold prompted args.
    ///
    /// # Errors
    /// As [`Self::of_fields`], for a struct variant's fields.
    pub(crate) fn of_variants(
        variants: &[VariantInput],
        naming: &SubcommandNaming,
    ) -> darling::Result<Self> {
        let mut errors = Error::accumulator();
        let branches = variants
            .iter()
            .filter_map(|variant| {
                errors.handle(PromptedArgBranch::of_variant(variant, naming)).flatten()
            })
            .collect();
        errors.finish_with(Self { branches, own_args: Vec::new() })
    }
}

impl PromptedArgBranch {
    /// A flattened or subcommand field's type, merged into its level.
    fn of_field(field: &FieldInput) -> Option<Self> {
        matches!(
            FieldKind::from(field.attrs.as_slice()),
            FieldKind::Flatten | FieldKind::Subcommand
        )
        .then(|| Self::Flattened(field.ty.peel_option().clone()))
    }

    /// The branch a variant hangs off its enum's tree, if it can hold prompted args.
    fn of_variant(
        variant: &VariantInput,
        naming: &SubcommandNaming,
    ) -> darling::Result<Option<Self>> {
        let attrs = variant.attrs.as_slice();
        if attrs.has_clap_key("skip") || attrs.has_clap_key("external_subcommand") {
            return Ok(None);
        }
        match variant.fields.style {
            Style::Struct => PromptedArgTree::of_fields(&variant.fields.fields)
                .map(|tree| Some(Self::Inline(naming.name_of(variant), tree))),
            Style::Tuple => Ok(variant.fields.fields.first().map(|field| {
                if attrs.has_clap_key("flatten") {
                    Self::Flattened(field.ty.clone())
                } else {
                    Self::Subcommand(naming.name_of(variant), field.ty.clone())
                }
            })),
            Style::Unit => Ok(None),
        }
    }
}

impl PromptAttr {
    fn new(field: &FieldInput, purpose: Purpose, question: &Question) -> darling::Result<Self> {
        let (span, attr) = (question.span(), purpose.attr());
        if !cfg!(feature = "interactive") {
            return Err(Error::custom(format!(
                "{attr} needs clap-couture's `interactive` feature; enable it or one of \
                 `interactive-cliclack`, `interactive-dialoguer`, `interactive-inquire`",
            ))
            .with_span(&span));
        }
        let Some(arg_id) = field.clap_arg_id() else {
            return Err(Error::custom(format!("{attr} needs a named field")).with_span(&span));
        };
        if purpose == Purpose::Confirmation && !is_bool(&field.ty) {
            return Err(Error::custom(format!("{attr} marks a `bool` flag")).with_span(&span));
        }
        let question = match &**question {
            Override::Inherit => None,
            Override::Explicit(question) => Some(question.clone()),
        };
        Ok(Self { arg_id, purpose, question })
    }

    /// The field's `#[couture(prompt)]` or `#[couture(confirm)]`, if it has one.
    fn of_field(field: &FieldInput) -> darling::Result<Option<Self>> {
        let (purpose, question) = match (&field.prompt, &field.confirm) {
            (None, None) => return Ok(None),
            (Some(prompt), None) => (Purpose::Value, prompt),
            (None, Some(confirm)) => (Purpose::Confirmation, confirm),
            (Some(_), Some(confirm)) => {
                return Err(Error::custom(
                    "`#[couture(prompt)]` and `#[couture(confirm)]` cannot mark the same field",
                )
                .with_span(&confirm.span()));
            }
        };
        let (span, attr) = (question.span(), purpose.attr());
        match FieldKind::from(field.attrs.as_slice()) {
            FieldKind::Arg => Self::new(field, purpose, question).map(Some),
            FieldKind::Flatten => Err(misplaced(span, attr, "the fields of the flattened type")),
            FieldKind::Skip => {
                Err(Error::custom(format!("{attr} cannot mark a field clap skips"))
                    .with_span(&span))
            }
            FieldKind::Subcommand => {
                Err(misplaced(span, attr, "the fields of the subcommand's variants"))
            }
        }
    }
}

impl Purpose {
    /// The attribute that declares it, as error messages name it.
    const fn attr(self) -> &'static str {
        match self {
            Self::Confirmation => "`#[couture(confirm)]`",
            Self::Value => "`#[couture(prompt)]`",
        }
    }
}

impl ToTokens for PromptedArgTree {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let (branches, own_args) = (&self.branches, &self.own_args);
        tokens.extend(quote! {
            ::clap_couture::interactive::PromptedArgTree {
                branches: &[#(#branches),*],
                own_args: &[#(#own_args),*],
            }
        });
    }
}

impl ToTokens for PromptedArgBranch {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(match self {
            Self::Flattened(flattened_type) => {
                let tree = maybe_couture_prompted_args(flattened_type);
                quote!(::clap_couture::interactive::PromptedArgBranch::Flattened(#tree))
            }
            Self::Inline(subcommand_name, tree) => quote! {
                ::clap_couture::interactive::PromptedArgBranch::Subcommand(
                    #subcommand_name,
                    &#tree,
                )
            },
            Self::Subcommand(subcommand_name, subcommand_type) => {
                let tree = maybe_couture_prompted_args(subcommand_type);
                quote! {
                    ::clap_couture::interactive::PromptedArgBranch::Subcommand(
                        #subcommand_name,
                        #tree,
                    )
                }
            }
        });
    }
}

impl ToTokens for PromptAttr {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let arg_id = &self.arg_id;
        let purpose = match self.purpose {
            Purpose::Confirmation => quote!(Confirmation),
            Purpose::Value => quote!(Value),
        };
        let question = OptionalText::from(&self.question);
        tokens.extend(quote! {
            ::clap_couture::interactive::PromptAttr {
                arg_id: #arg_id,
                purpose: ::clap_couture::interactive::PromptPurpose::#purpose,
                question: #question,
            }
        });
    }
}

/// Check whether `field_type` is spelled `bool`, which clap makes a flag.
fn is_bool(field_type: &Type) -> bool {
    matches!(field_type, Type::Path(type_path) if type_path.qself.is_none() && type_path.path.is_ident("bool"))
}

fn misplaced(span: Span, attr: &str, instead: &str) -> Error {
    Error::custom(format!("{attr} marks a single arg; put it on {instead} instead"))
        .with_span(&span)
}

/// `field_type`'s prompted args, or none when `field_type` does not implement `Couture`.
fn maybe_couture_prompted_args(field_type: &Type) -> TokenStream2 {
    quote!(<::clap_couture::MaybeCouture<#field_type>>::PROMPTED_ARGS)
}
