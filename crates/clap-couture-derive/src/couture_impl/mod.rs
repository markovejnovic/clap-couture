//! The `impl clap_couture::Couture` one derive emits.

mod categories;
mod prompted_args;

use darling::ast;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{Generics, Ident};

use self::{categories::Categories, prompted_args::PromptedArgTree};
use crate::{clap_compat::SubcommandNaming, input::CoutureInput};

/// The `impl clap_couture::Couture` one derive emits.
pub(crate) struct CoutureImpl {
    /// `None` keeps the trait's empty default: a parser struct without a subcommand.
    categories: Option<Categories>,
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
        let (categories, prompted_arg_tree) = match &input.data {
            ast::Data::Enum(variants) => {
                let naming = SubcommandNaming::from_container(&input.attrs);
                let categories = Categories::parse(&input.attrs, variants, &naming)?;
                (Some(categories), PromptedArgTree::of_variants(variants, &naming)?)
            }
            ast::Data::Struct(fields) => (
                Categories::of_subcommand_field(&fields.fields),
                PromptedArgTree::of_fields(&fields.fields)?,
            ),
        };
        Ok(Self {
            categories,
            prompted_arg_tree,
            type_generics: input.generics,
            type_name: input.ident,
        })
    }
}

impl ToTokens for CoutureImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let (type_name, prompted_arg_tree) = (&self.type_name, &self.prompted_arg_tree);
        let (impl_parameters, type_arguments, where_clause) = self.type_generics.split_for_impl();
        let categories_const = self.categories.as_ref().map(|categories| {
            quote!(const CATEGORIES: ::clap_couture::CommandCategoryMap = #categories;)
        });

        let prompted_arg_tree_const = cfg!(feature = "interactive").then(|| {
            quote! {
                const PROMPTED_ARGS: ::clap_couture::interactive::PromptedArgTree = {
                    use ::clap_couture::interactive::NotCouture as _;

                    #prompted_arg_tree
                };
            }
        });

        tokens.extend(quote! {
            impl #impl_parameters ::clap_couture::Couture
                for #type_name #type_arguments #where_clause
            {
                #categories_const
                #prompted_arg_tree_const
            }
        });
    }
}
