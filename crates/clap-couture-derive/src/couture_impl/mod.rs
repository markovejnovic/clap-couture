//! The `impl clap_couture::Couture` one derive emits.

mod categories;
mod container;
mod help_tree;
mod messages;
mod prompted_args;

use darling::ast;
use proc_macro2::{TokenStream as TokenStream2, TokenTree};
use quote::{ToTokens, quote};
use syn::{GenericParam, Generics, Ident, Path, Type, WherePredicate, parse_quote};

use self::{
    categories::Categories, container::ContainerAttrs, help_tree::HelpBranches, messages::Messages,
    prompted_args::PromptedArgTree,
};
use crate::{clap_compat::SubcommandNaming, input::CoutureInput, runtime::Runtime};

/// The `impl clap_couture::Couture` one derive emits.
pub(crate) struct CoutureImpl {
    /// The categories of its subcommands, its own and its flattened types'.
    categories: Categories,
    /// `#[couture(crate = ...)]`, if the type renames clap-couture.
    crate_path: Option<Path>,
    /// The commands below the type's, for their help.
    help_branches: HelpBranches,
    /// The text couture prints itself, if the type sets any.
    messages: Option<Messages>,
    /// The args marked `#[couture(prompt)]`, from the deriving type down.
    prompted_arg_tree: PromptedArgTree,
    /// The deriving type's generic parameters and where clause.
    type_generics: Generics,
    /// The name of the type the derive is on.
    type_name: Ident,
}

impl TryFrom<CoutureInput> for CoutureImpl {
    type Error = darling::Error;

    fn try_from(input: CoutureInput) -> darling::Result<Self> {
        let mut container = ContainerAttrs::parse(&input.attrs)?;
        let (categories, help_branches, prompted_arg_tree) = match &input.data {
            ast::Data::Enum(variants) => {
                let naming = SubcommandNaming::from_container(&input.attrs);
                let categories = Categories::parse(&mut container, variants, &naming)?;
                let help_branches = HelpBranches::of_variants(variants, &naming);
                (categories, help_branches, PromptedArgTree::of_variants(variants, &naming)?)
            }
            ast::Data::Struct(fields) => {
                container.refuse_enum_only_keys()?;
                (
                    Categories::of_args(&fields.fields),
                    HelpBranches::of_fields(&fields.fields),
                    PromptedArgTree::of_fields(&fields.fields)?,
                )
            }
        };
        Ok(Self {
            categories,
            crate_path: container.crate_path,
            help_branches,
            messages: container.messages,
            prompted_arg_tree,
            type_generics: input.generics,
            type_name: input.ident,
        })
    }
}

impl CoutureImpl {
    /// The type's generics, plus a `Couture` bound on each flattened or subcommand type naming a
    /// type parameter, so a generic one keeps its consts rather than falling back to none.
    fn bounded_generics(&self) -> Generics {
        let mut generics = self.type_generics.clone();
        let bounds: Vec<_> = self
            .prompted_arg_tree
            .merged_types()
            .into_iter()
            .filter(|merged_type| self.names_type_parameter(merged_type))
            .map(|merged_type| -> WherePredicate { parse_quote!(#merged_type: #Runtime::Couture) })
            .collect();
        generics.make_where_clause().predicates.extend(bounds);
        generics
    }

    /// Whether `ty` names one of the type's type parameters anywhere in its tokens.
    fn names_type_parameter(&self, ty: &Type) -> bool {
        let parameters: Vec<&Ident> = self
            .type_generics
            .params
            .iter()
            .filter_map(|parameter| match parameter {
                GenericParam::Type(type_parameter) => Some(&type_parameter.ident),
                GenericParam::Lifetime(_) | GenericParam::Const(_) => None,
            })
            .collect();
        let mut token_streams = Vec::from([ty.to_token_stream()]);
        while let Some(token_stream) = token_streams.pop() {
            for token in token_stream {
                match token {
                    TokenTree::Ident(ident) if parameters.contains(&&ident) => return true,
                    TokenTree::Group(group) => token_streams.push(group.stream()),
                    TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
                }
            }
        }
        false
    }
}

impl ToTokens for CoutureImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let (type_name, categories, help_branches, prompted_arg_tree) =
            (&self.type_name, &self.categories, &self.help_branches, &self.prompted_arg_tree);
        let bounded_generics = self.bounded_generics();
        let (impl_parameters, type_arguments, where_clause) = bounded_generics.split_for_impl();
        let crate_path = self
            .crate_path
            .as_ref()
            .map_or_else(|| quote!(::clap_couture), ToTokens::to_token_stream);

        let messages_const = self
            .messages
            .as_ref()
            .map(|messages| quote!(const MESSAGES: #Runtime::Messages = #messages;));
        let own_messages = self.messages.as_ref().map_or_else(
            || quote!(::core::option::Option::None),
            |_| quote!(::core::option::Option::Some(<Self as #Runtime::Couture>::MESSAGES)),
        );

        let prompted_arg_tree_const = cfg!(feature = "interactive").then(|| {
            quote! {
                const PROMPTED_ARGS: #Runtime::__private::PromptedArgTree = {
                    use #Runtime::__private::NotCouture as _;

                    // Fails the build when one command path reads stdin twice, across types.
                    #prompted_arg_tree.assert_stdin_read_once();
                    #prompted_arg_tree
                };
            }
        });

        // The block binds the crate to one alias, so a renamed dependency only changes the `use`.
        tokens.extend(quote! {
            const _: () = {
                use #crate_path as #Runtime;

                impl #impl_parameters #Runtime::Couture for #type_name #type_arguments #where_clause {
                    const CATEGORIES: #Runtime::CommandCategoryMap = {
                        use #Runtime::__private::NotCouture as _;

                        #categories
                    };
                    #messages_const
                    const HELP_TREE: #Runtime::__private::HelpTree = {
                        use #Runtime::__private::NotCouture as _;

                        #Runtime::__private::HelpTree {
                            branches: #help_branches,
                            categories: <Self as #Runtime::Couture>::CATEGORIES,
                            messages: #own_messages,
                        }
                    };
                    #prompted_arg_tree_const
                }
            };
        });
    }
}
