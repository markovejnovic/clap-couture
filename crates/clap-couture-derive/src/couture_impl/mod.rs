//! The `impl clap_couture::Couture` one derive emits.

mod categories;
mod messages;
mod prompted_args;

use darling::ast;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{Generics, Ident};

use self::{
    categories::{Categories, Spec},
    messages::Messages,
    prompted_args::PromptedArgTree,
};
use crate::{clap_compat::SubcommandNaming, input::CoutureInput};

/// The `impl clap_couture::Couture` one derive emits.
pub(crate) struct CoutureImpl {
    /// The categories of its subcommands, its own and its flattened types'.
    categories: Categories,
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
        let mut specs = Spec::parse_all(&input.attrs)?;
        // The last `messages = { ... }` wins outright, like `inherit`.
        let messages = specs.iter_mut().rev().find_map(|spec| spec.messages.take());
        let (categories, prompted_arg_tree) = match &input.data {
            ast::Data::Enum(variants) => {
                let naming = SubcommandNaming::from_container(&input.attrs);
                let categories = Categories::parse(specs, variants, &naming)?;
                (categories, PromptedArgTree::of_variants(variants, &naming)?)
            }
            ast::Data::Struct(fields) => {
                (Categories::of_args(&fields.fields), PromptedArgTree::of_fields(&fields.fields)?)
            }
        };
        Ok(Self {
            categories,
            messages,
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

        let messages_const = self
            .messages
            .as_ref()
            .map(|messages| quote!(const MESSAGES: ::clap_couture::Messages = #messages;));

        let prompted_arg_tree_const = cfg!(feature = "interactive").then(|| {
            quote! {
                const PROMPTED_ARGS: ::clap_couture::interactive::PromptedArgTree = {
                    use ::clap_couture::NotCouture as _;

                    #prompted_arg_tree
                };
            }
        });

        tokens.extend(quote! {
            impl #impl_parameters ::clap_couture::Couture
                for #type_name #type_arguments #where_clause
            {
                const CATEGORIES: ::clap_couture::CommandCategoryMap = {
                    use ::clap_couture::NotCouture as _;

                    #categories
                };
                #messages_const
                #prompted_arg_tree_const
            }
        });
    }
}
