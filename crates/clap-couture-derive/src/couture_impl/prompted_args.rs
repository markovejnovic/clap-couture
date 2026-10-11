//! `PROMPTED_ARGS`: the args `#[couture(prompt)]`, `#[couture(confirm)]` and
//! `#[couture(stdin)]` mark, and the command path to each.

use darling::{Error, ast::Style, util::Override};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::Type;

use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, ClapName, FieldKind, SubcommandNaming},
    input::{BareOr, EditorOptions, FieldInput, StdinOptions, VariantInput},
    runtime::Runtime,
    text::{OptionalText, TextExpr},
};

/// The prompted args of one command level, and the tree below it.
pub(crate) struct PromptedArgTree {
    /// Merged trees and subcommands.
    branches: Vec<PromptedArgBranch>,
    /// The args prompted for at this level.
    own_args: Vec<PromptAttr>,
    /// This level's `#[couture(stdin)]` arg, if any.
    stdin_arg: Option<StdinAttr>,
}

/// What hangs off one command level besides its own prompted args.
enum PromptedArgBranch {
    /// A type merged into this level: a flattened `Args` type, or the subcommand enum.
    Flattened(Type),
    /// A struct variant: a subcommand whose args are its own fields.
    Inline(ClapName, PromptedArgTree),
    /// A tuple variant: a subcommand whose args are another type's.
    Subcommand(ClapName, Type),
}

/// A `dash` in `prompt(dash)`, or a `stdin` without `piped`.
struct DashAttr {
    /// The field's type, `Option` peeled: what `-` becomes.
    field_type: Type,
}

/// The `dash` of a `PromptAttr` or `StdinAttr`, emitted as an `Option`.
struct OptionalDash<'dash>(Option<&'dash DashAttr>);

/// The `editor` beside a `#[couture(prompt)]`.
struct EditorAttr {
    /// The temp file's extension, without the dot.
    extension: String,
    /// The text the file starts with when the arg has no default.
    initial: Option<TextExpr>,
}

/// One `#[couture(prompt)]` or `#[couture(confirm)]`, with or without `= "..."`.
struct PromptAttr {
    /// The clap id of the arg it is on.
    arg_id: ClapName,
    /// `prompt(dash)`: asked only when `-` is its value.
    dash: Option<DashAttr>,
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
/// A field's `#[couture(stdin)]`.
struct StdinAttr {
    /// The clap id of the arg it is on.
    arg_id: ClapName,
    /// Stdin is read only when `-` is its value; `None` for `stdin(piped)`.
    dash: Option<DashAttr>,
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
    /// - A `#[couture(stdin)]` on more than one field, or on one that is not a single named arg
    ///   taking one value.
    /// - A `dash` on an arg that does not take one value.
    pub(crate) fn of_fields(fields: &[FieldInput]) -> darling::Result<Self> {
        let mut errors = Error::accumulator();
        let own_args: Vec<PromptAttr> = fields
            .iter()
            .filter_map(|field| errors.handle(PromptAttr::of_field(field)).flatten())
            .collect();
        for prompt_attr in &own_args {
            for error in prompt_attr.secrets_named(&own_args) {
                errors.push(error);
            }
        }
        let branches = fields.iter().filter_map(PromptedArgBranch::of_field).collect();
        let mut marked_stdin =
            fields.iter().filter_map(|field| field.stdin.as_ref().map(|attr| (field, attr)));
        let stdin_arg = marked_stdin
            .next()
            .and_then(|(field, attr)| errors.handle(StdinAttr::new(field, attr)));
        for (_, stdin) in marked_stdin {
            errors.push(
                Error::custom(
                    "`#[couture(stdin)]` marks one arg per command; stdin can only be read once",
                )
                .with_span(&stdin.span()),
            );
        }
        errors.finish_with(Self { branches, own_args, stdin_arg })
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
        errors.finish_with(Self { branches, own_args: Vec::new(), stdin_arg: None })
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
        quote!(<#Runtime::__private::MaybeCouture<#field_type>>::PROMPTED_ARGS)
    }
}

impl EditorAttr {
    /// # Errors
    /// - `editor` without the `interactive-editor` feature.
    /// - `editor` on a `bool` flag, which has no value to edit; a `bool` given an `action` may take
    ///   one.
    /// - An `extension` that is empty, starts with a dot or holds a path separator.
    fn new(field: &FieldInput, editor: &BareOr<EditorOptions>) -> darling::Result<Self> {
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
        let initial = options.initial;
        let Some(extension) = options.extension else {
            return Ok(Self { extension: "txt".to_owned(), initial });
        };
        if extension.is_empty() || extension.starts_with('.') || extension.contains(['/', '\\']) {
            return Err(Error::custom(
                "`extension` is the temp file's extension without the dot, like `\"md\"`",
            )
            .with_span(&extension.span()));
        }
        Ok(Self { extension: extension.into_inner(), initial })
    }
}

impl PromptAttr {
    /// `dash` is the span of `prompt(dash)`, if given.
    fn new(
        field: &FieldInput,
        purpose: Purpose,
        span: Span,
        question: Option<&TextExpr>,
        dash: Option<Span>,
    ) -> darling::Result<Self> {
        let attr = purpose.attr();
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
        let dash = dash.map(|span| DashAttr::new(field, span)).transpose()?;
        let editor =
            field.editor.as_ref().map(|editor| EditorAttr::new(field, editor)).transpose()?;
        let question = question.cloned();
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
        Ok(Self { arg_id, dash, editor, purpose, question, secret })
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
        let (purpose, span, question, dash) = match (&field.prompt, &field.confirm) {
            (None, None) => return Ok(None),
            (Some(prompt), None) => {
                let dash = prompt.dash.is_present().then(|| prompt.dash.span());
                (Purpose::Value, prompt.span(), prompt.question.as_ref(), dash)
            }
            (None, Some(confirm)) => {
                let question = match &**confirm {
                    Override::Inherit => None,
                    Override::Explicit(question) => Some(question),
                };
                (Purpose::Confirmation, confirm.span(), question, None)
            }
            (Some(_), Some(confirm)) => {
                return Err(Error::custom(
                    "`#[couture(prompt)]` and `#[couture(confirm)]` cannot mark the same field",
                )
                .with_span(&confirm.span()));
            }
        };
        let attr = purpose.attr();
        match FieldKind::from(field.attrs.as_slice()) {
            FieldKind::Arg => Self::new(field, purpose, span, question, dash).map(Some),
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

    /// An error for each secret among `level_args` its confirmation's question names, whose
    /// value the confirmation would show.
    fn secrets_named(&self, level_args: &[Self]) -> Vec<Error> {
        let Some(question) =
            self.question.as_ref().filter(|_| self.purpose == Purpose::Confirmation)
        else {
            return Vec::new();
        };
        let is_secret = |id: &str| {
            level_args.iter().any(|arg| {
                arg.secret.is_some()
                    && matches!(&arg.arg_id, ClapName::Literal(secret_id) if secret_id == id)
            })
        };
        question
            .named_ids()
            .into_iter()
            .filter(|id| is_secret(id))
            .map(|id| {
                Error::custom(format!(
                    "`{{{id}}}` names a `#[couture(secret)]`; the confirmation would show its \
                     value"
                ))
                .with_span(&question.span())
            })
            .collect()
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
        let stdin_arg = self.stdin_arg.as_ref().map_or_else(
            || quote!(::core::option::Option::None),
            |StdinAttr { arg_id, dash }| {
                let dash = OptionalDash(dash.as_ref());
                quote! {
                    ::core::option::Option::Some(#Runtime::__private::StdinAttr {
                        arg_id: #arg_id,
                        dash: #dash,
                    })
                }
            },
        );
        tokens.extend(quote! {
            #Runtime::__private::PromptedArgTree {
                branches: &[#(#branches),*],
                own_args: &[#(#own_args),*],
                stdin_arg: #stdin_arg,
            }
        });
    }
}

impl ToTokens for PromptedArgBranch {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(match self {
            Self::Flattened(flattened_type) => {
                let tree = Self::prompted_args_of(flattened_type);
                quote!(#Runtime::__private::PromptedArgBranch::Flattened(#tree))
            }
            Self::Inline(subcommand_name, tree) => quote! {
                #Runtime::__private::PromptedArgBranch::Subcommand(
                    #subcommand_name,
                    &#tree,
                )
            },
            Self::Subcommand(subcommand_name, subcommand_type) => {
                let tree = Self::prompted_args_of(subcommand_type);
                quote! {
                    #Runtime::__private::PromptedArgBranch::Subcommand(
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
        let (arg_id, dash) = (&self.arg_id, OptionalDash(self.dash.as_ref()));
        let editor = self.editor.as_ref().map_or_else(
            || quote!(::core::option::Option::None),
            |EditorAttr { extension, initial }| {
                let initial = OptionalText::from(initial);
                quote! {
                    ::core::option::Option::Some({
                        // Fails the build when clap-couture can't open an editor.
                        const _: () = #Runtime::__private::EditorAttr::FEATURE_CHECK;
                        #Runtime::__private::EditorAttr {
                            extension: #extension,
                            initial: #initial,
                        }
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
                        #Runtime::__private::SecretAttr::new::<#secret_type>(#allow_argv)
                    )
                }
            },
        );
        tokens.extend(quote! {
            #Runtime::__private::PromptAttr {
                arg_id: #arg_id,
                dash: #dash,
                editor: #editor,
                purpose: #Runtime::__private::PromptPurpose::#purpose,
                question: #question,
                secret: #secret,
            }
        });
    }
}

impl StdinAttr {
    /// `field`'s `#[couture(stdin)]`, `stdin`.
    ///
    /// # Errors
    /// - No `interactive` feature.
    /// - The field is not a single named arg, takes no value, or takes many.
    /// - `#[couture(confirm)]` beside it.
    fn new(field: &FieldInput, stdin: &BareOr<StdinOptions>) -> darling::Result<Self> {
        let span = stdin.span();
        let error = |message: &str| Err(Error::custom(message).with_span(&span));
        if !cfg!(feature = "interactive") {
            return error(
                "`#[couture(stdin)]` needs clap-couture's `interactive` feature; enable it or one \
                 of `interactive-cliclack`, `interactive-dialoguer`, `interactive-inquire`",
            );
        }
        match FieldKind::from(field.attrs.as_slice()) {
            FieldKind::Arg => {}
            FieldKind::Flatten | FieldKind::Subcommand => {
                return error(
                    "`#[couture(stdin)]` marks a single arg; put it on a field of that type \
                     instead",
                );
            }
            FieldKind::Skip => {
                return error("`#[couture(stdin)]` cannot mark a field clap skips");
            }
        }
        let Some(arg_id) = field.clap_arg_id() else {
            return error("`#[couture(stdin)]` needs a named field");
        };
        if field.confirm.is_some() || field.ty.is_bool() && !field.attrs.has_clap_key("action") {
            return error("`#[couture(stdin)]` needs an arg that takes a value");
        }
        if field.ty.is_multi_value() {
            return error("`#[couture(stdin)]` takes a single value");
        }
        let dash = match &**stdin {
            Override::Explicit(StdinOptions { piped }) if piped.is_present() => None,
            Override::Inherit | Override::Explicit(_) => Some(DashAttr::new(field, span)?),
        };
        Ok(Self { arg_id, dash })
    }
}

impl DashAttr {
    /// The `dash` at `span` on `field`.
    ///
    /// # Errors
    /// The field is a flag that takes no value, or takes many.
    fn new(field: &FieldInput, span: Span) -> darling::Result<Self> {
        if field.ty.is_bool() && !field.attrs.has_clap_key("action") {
            return Err(Error::custom("`dash` needs an arg that takes a value").with_span(&span));
        }
        if field.ty.is_multi_value() {
            return Err(
                Error::custom("`dash` needs an arg that takes a single value").with_span(&span)
            );
        }
        Ok(Self { field_type: field.ty.peel_option().clone() })
    }
}

impl ToTokens for OptionalDash<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        // The type keeps the user's span, so a field clap can't hand over is the error's location.
        tokens.extend(self.0.map_or_else(
            || quote!(::core::option::Option::None),
            |DashAttr { field_type }| {
                quote! {
                    ::core::option::Option::Some(
                        #Runtime::__private::DashAttr::new::<#field_type>()
                    )
                }
            },
        ));
    }
}
