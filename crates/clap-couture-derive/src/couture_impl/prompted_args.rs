//! `PROMPTED_ARGS`: the args `#[couture(prompt)]` and `#[couture(confirm)]` mark, and the command
//! path to each.

use darling::{
    Error,
    ast::Style,
    util::{Override, SpannedValue},
};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::Type;

use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, FieldKind, SubcommandNaming},
    input::{EditorOptions, FieldInput, Question, VariantInput},
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

/// The `editor` beside a `#[couture(prompt)]`.
struct EditorAttr {
    /// The temp file's extension, without the dot.
    extension: String,
}

/// One `#[couture(prompt)]` or `#[couture(confirm)]`, with or without `= "..."`.
struct PromptAttr {
    /// The clap id of the arg it is on.
    arg_id: String,
    /// `Some` for `#[couture(prompt, editor)]`.
    editor: Option<EditorAttr>,
    /// What the answer is for.
    purpose: Purpose,
    /// The `= ...` override; `None` asks the arg's help text.
    question: Option<TextExpr>,
    /// The `secret` beside it, if any.
    secret: Option<SecretAttr>,
}

/// What a prompt's answer is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Confirmation,
    Value,
}
/// A `secret` beside a prompt, and the type it must hold.
struct SecretAttr {
    /// `secret(allow_argv)`.
    allow_argv: bool,
    /// The field's type, `Option` peeled, checked against `SecretField`.
    secret_type: Type,
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

    /// `field_type`'s prompted args, or none when `field_type` does not implement `Couture`.
    fn prompted_args_of(field_type: &Type) -> TokenStream2 {
        quote!(<::clap_couture::MaybeCouture<#field_type>>::PROMPTED_ARGS)
    }
}

impl EditorAttr {
    /// # Errors
    /// - `editor` without the `interactive-editor` feature.
    /// - `editor` on a `bool` flag, which has no value to edit; a `bool` given an `action` may take
    ///   one.
    /// - An `extension` that is empty, starts with a dot or holds a path separator.
    fn new(
        field: &FieldInput,
        editor: &SpannedValue<Override<EditorOptions>>,
    ) -> darling::Result<Self> {
        let span = editor.span();
        if !cfg!(feature = "interactive-editor") {
            return Err(Error::custom(
                "`#[couture(editor)]` needs clap-couture's `interactive-editor` feature",
            )
            .with_span(&span));
        }
        if field.ty.is_bool() && !field.attrs.has_clap_key("action") {
            return Err(Error::custom("`#[couture(editor)]` needs an arg that takes a value")
                .with_span(&span));
        }
        if field.ty.is_multi_value() {
            return Err(Error::custom(
                "`#[couture(editor)]` takes a single value; drop `editor` to ask for each value \
                 in turn",
            )
            .with_span(&span));
        }
        let options = match &**editor {
            Override::Inherit => EditorOptions::default(),
            Override::Explicit(options) => options.clone(),
        };
        let Some(extension) = options.extension else {
            return Ok(Self { extension: "txt".to_owned() });
        };
        if extension.is_empty() || extension.starts_with('.') || extension.contains(['/', '\\']) {
            return Err(Error::custom(
                "`extension` is the temp file's extension without the dot, like `\"md\"`",
            )
            .with_span(&extension.span()));
        }
        Ok(Self { extension: extension.into_inner() })
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
        if purpose == Purpose::Confirmation && !field.ty.is_bool() {
            return Err(Error::custom(format!("{attr} marks a `bool` flag")).with_span(&span));
        }
        let editor =
            field.editor.as_ref().map(|editor| EditorAttr::new(field, editor)).transpose()?;
        let question = match &**question {
            Override::Inherit => None,
            Override::Explicit(question) => Some(question.clone()),
        };
        let secret = field
            .secret
            .as_ref()
            .map(|secret| {
                if let Some(default_span) = field.attrs.clap_default_span() {
                    return Err(Error::custom(
                        "a `#[couture(secret)]` can't have a default: it would be baked into the \
                         binary and shown in `--help`",
                    )
                    .with_span(&default_span));
                }
                let allow_argv = match &**secret {
                    Override::Inherit => false,
                    Override::Explicit(options) => options.allow_argv.is_present(),
                };
                Ok(SecretAttr { allow_argv, secret_type: field.ty.peel_option().clone() })
            })
            .transpose()?;
        Ok(Self { arg_id, editor, purpose, question, secret })
    }

    /// The field's `#[couture(prompt)]` or `#[couture(confirm)]`, if it has one.
    fn of_field(field: &FieldInput) -> darling::Result<Option<Self>> {
        if let (None, Some(secret)) = (&field.prompt, &field.secret) {
            return Err(Error::custom(
                "`#[couture(secret)]` modifies `#[couture(prompt)]`; add `prompt` beside it",
            )
            .with_span(&secret.span()));
        }
        if let (Some(secret), Some(_)) = (&field.secret, &field.editor) {
            return Err(Error::custom(
                "`#[couture(secret)]` can't be asked in the editor: the answer would sit in a \
                 temp file in plain text",
            )
            .with_span(&secret.span()));
        }
        if let (None, Some(editor)) = (&field.prompt, &field.editor) {
            return Err(Error::custom(
                "`#[couture(editor)]` modifies `#[couture(prompt)]`; add `prompt` beside it",
            )
            .with_span(&editor.span()));
        }
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
            FieldKind::Flatten => Err(purpose.misplaced(span, "the fields of the flattened type")),
            FieldKind::Skip => {
                Err(Error::custom(format!("{attr} cannot mark a field clap skips"))
                    .with_span(&span))
            }
            FieldKind::Subcommand => {
                Err(purpose.misplaced(span, "the fields of the subcommand's variants"))
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

    fn misplaced(self, span: Span, instead: &str) -> Error {
        let attr = self.attr();
        Error::custom(format!("{attr} marks a single arg; put it on {instead} instead"))
            .with_span(&span)
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
                let tree = Self::prompted_args_of(flattened_type);
                quote!(::clap_couture::interactive::PromptedArgBranch::Flattened(#tree))
            }
            Self::Inline(subcommand_name, tree) => quote! {
                ::clap_couture::interactive::PromptedArgBranch::Subcommand(
                    #subcommand_name,
                    &#tree,
                )
            },
            Self::Subcommand(subcommand_name, subcommand_type) => {
                let tree = Self::prompted_args_of(subcommand_type);
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
        let editor = self.editor.as_ref().map_or_else(
            || quote!(::core::option::Option::None),
            |EditorAttr { extension }| {
                quote! {
                    ::core::option::Option::Some({
                        // Fails the build when clap-couture can't open an editor.
                        const _: () = ::clap_couture::interactive::EditorAttr::FEATURE_CHECK;
                        ::clap_couture::interactive::EditorAttr { extension: #extension }
                    })
                }
            },
        );
        let purpose = match self.purpose {
            Purpose::Confirmation => quote!(Confirmation),
            Purpose::Value => quote!(Value),
        };
        let question = OptionalText::from(&self.question);
        // The type keeps the user's span, so a non-`SecretString` field is the error's location.
        let secret = self.secret.as_ref().map_or_else(
            || quote!(::core::option::Option::None),
            |SecretAttr { allow_argv, secret_type }| {
                quote! {
                    ::core::option::Option::Some(
                        ::clap_couture::interactive::SecretAttr::new::<#secret_type>(#allow_argv)
                    )
                }
            },
        );
        tokens.extend(quote! {
            ::clap_couture::interactive::PromptAttr {
                arg_id: #arg_id,
                editor: #editor,
                purpose: ::clap_couture::interactive::PromptPurpose::#purpose,
                question: #question,
                secret: #secret,
            }
        });
    }
}
