//! `HELP_TREE`: the commands below the deriving type's, so each gets couture's help from the type
//! that defines it.

use darling::ast::Style;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::Type;

use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, ClapName, FieldKind, SubcommandNaming},
    input::{FieldInput, VariantInput},
    runtime::Runtime,
};

/// What hangs off one command level, as `&[clap_couture::__private::HelpBranch]`.
pub(crate) struct HelpBranches(Vec<HelpBranch>);

/// One branch of a command level.
enum HelpBranch {
    /// A type merged into this level: a flattened `Args` type, or the subcommand enum.
    Flattened(Type),
    /// A struct variant: a subcommand whose subcommands come from its own fields.
    Inline(ClapName, HelpBranches),
    /// A tuple variant's subcommand, defined by its type, or a unit variant's, by nothing.
    Subcommand(ClapName, Option<Type>),
}

impl HelpBranches {
    /// The `CommandCategoryMap` of a struct variant's command: those of the types merged in.
    fn categories(&self) -> TokenStream2 {
        let merged_maps = self.0.iter().filter_map(|branch| match branch {
            HelpBranch::Flattened(merged_type) => {
                Some(quote!(<#Runtime::__private::MaybeCouture<#merged_type>>::CATEGORIES))
            }
            HelpBranch::Inline(..) | HelpBranch::Subcommand(..) => None,
        });
        quote! {
            #Runtime::CommandCategoryMap::EMPTY.with_flattened_maps(&[#(#merged_maps),*])
        }
    }

    /// The branches of a struct's fields: its subcommand enum first, then the `Args` types it
    /// flattens, `Option` peeled.
    pub(crate) fn of_fields(fields: &[FieldInput]) -> Self {
        let types_of = |wanted_kind: FieldKind| {
            fields
                .iter()
                .filter(move |field| FieldKind::from(field.attrs.as_slice()) == wanted_kind)
                .map(|field| HelpBranch::Flattened(field.ty.peel_option().clone()))
        };
        Self(types_of(FieldKind::Subcommand).chain(types_of(FieldKind::Flatten)).collect())
    }

    /// The branches of a subcommand enum: one per variant clap makes a command of, or merges in.
    pub(crate) fn of_variants(variants: &[VariantInput], naming: &SubcommandNaming) -> Self {
        Self(
            variants.iter().filter_map(|variant| HelpBranch::of_variant(variant, naming)).collect(),
        )
    }
}

impl HelpBranch {
    /// `couture_type`'s help tree, or an empty one when it does not implement `Couture`.
    fn help_tree_of(couture_type: &Type) -> TokenStream2 {
        quote!(<#Runtime::__private::MaybeCouture<#couture_type>>::HELP_TREE)
    }

    /// The branch a variant hangs off its enum's tree, if clap makes a command of it.
    fn of_variant(variant: &VariantInput, naming: &SubcommandNaming) -> Option<Self> {
        let attrs = variant.attrs.as_slice();
        if attrs.has_clap_key("skip") || attrs.has_clap_key("external_subcommand") {
            return None;
        }
        let name = naming.name_of(variant);
        let first_type = variant.fields.fields.first().map(|field| field.ty.clone());
        Some(match variant.fields.style {
            Style::Struct => Self::Inline(name, HelpBranches::of_fields(&variant.fields.fields)),
            Style::Tuple if attrs.has_clap_key("flatten") => Self::Flattened(first_type?),
            Style::Tuple | Style::Unit => Self::Subcommand(name, first_type),
        })
    }
}

impl ToTokens for HelpBranches {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let branches = &self.0;
        tokens.extend(quote!(&[#(#branches),*]));
    }
}

impl ToTokens for HelpBranch {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(match self {
            Self::Flattened(merged_type) => {
                let tree = Self::help_tree_of(merged_type);
                quote!(#Runtime::__private::HelpBranch::Flattened(#tree))
            }
            Self::Inline(subcommand_name, branches) => {
                let categories = branches.categories();
                quote! {
                    #Runtime::__private::HelpBranch::Subcommand(
                        #subcommand_name,
                        &#Runtime::__private::HelpTree {
                            branches: #branches,
                            categories: #categories,
                            messages: ::core::option::Option::None,
                        },
                    )
                }
            }
            Self::Subcommand(subcommand_name, subcommand_type) => {
                let tree = subcommand_type.as_ref().map_or_else(
                    || quote!(&#Runtime::__private::HelpTree::EMPTY),
                    Self::help_tree_of,
                );
                quote!(#Runtime::__private::HelpBranch::Subcommand(#subcommand_name, #tree))
            }
        });
    }
}
