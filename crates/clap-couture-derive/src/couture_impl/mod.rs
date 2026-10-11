//! The `impl clap_couture::Couture` one derive emits.

mod categories;
mod container;
mod messages;
mod prompted_args;

use darling::ast;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{Generics, Ident, Path};

use self::{
    categories::Categories, container::ContainerAttrs, messages::Messages,
    prompted_args::PromptedArgTree,
};
use crate::{clap_compat::SubcommandNaming, input::CoutureInput, runtime::Runtime};

/// The `impl clap_couture::Couture` one derive emits.
pub(crate) struct CoutureImpl {
    /// The categories of its subcommands, its own and its flattened types'.
    categories: Categories,
    /// `#[couture(crate = ...)]`, if the type renames clap-couture.
    crate_path: Option<Path>,
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
        let (categories, prompted_arg_tree) = match &input.data {
            ast::Data::Enum(variants) => {
                let naming = SubcommandNaming::from_container(&input.attrs);
                let categories = Categories::parse(&mut container, variants, &naming)?;
                (categories, PromptedArgTree::of_variants(variants, &naming)?)
            }
            ast::Data::Struct(fields) => {
                container.refuse_enum_only_keys()?;
                (Categories::of_args(&fields.fields), PromptedArgTree::of_fields(&fields.fields)?)
            }
        };
        Ok(Self {
            categories,
            crate_path: container.crate_path,
            messages: container.messages,
            prompted_arg_tree,
            type_generics: input.generics,
            type_name: input.ident,
        })
    }
}

impl ToTokens for CoutureImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let (type_name, categories, prompted_arg_tree) =
            (&self.type_name, &self.categories, &self.prompted_arg_tree);
        let (impl_parameters, type_arguments, where_clause) = self.type_generics.split_for_impl();
        let crate_path = self
            .crate_path
            .as_ref()
            .map_or_else(|| quote!(::clap_couture), ToTokens::to_token_stream);

        let messages_const = self
            .messages
            .as_ref()
            .map(|messages| quote!(const MESSAGES: #Runtime::Messages = #messages;));

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
                    #prompted_arg_tree_const
                }
            };
        });
    }
}
