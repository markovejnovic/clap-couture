//! `CATEGORIES`: the category each subcommand is filed under.

use darling::Error;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{LitStr, Type};

use super::container::{CategorySpec, ContainerAttrs, Inheritance};
use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, ClapName, FieldKind, SubcommandNaming},
    input::{FieldInput, VariantInput},
    runtime::Runtime,
    text::TextExpr,
};

/// One category, as `clap_couture::Category`.
struct Category {
    description: Option<TextExpr>,
    label: String,
    title: Option<TextExpr>,
}

/// A category, and the subcommands filed under it.
pub(crate) struct Section {
    category: Category,
    commands: Vec<ClapName>,
}

/// The categories an `impl Couture` reports.
pub(crate) enum Categories {
    /// A parser or `Args` struct's. It has no subcommands of its own, so its categories are its
    /// subcommand enum's, then those of the `Args` types it flattens.
    Args {
        /// The `Args` types of its `#[command(flatten)]` fields, in field order. They need not
        /// implement `Couture`: one from another crate brings no categories.
        flattened_args: Vec<Type>,
        /// The subcommand enum of its `#[command(subcommand)]` field, if any.
        subcommand_enum: Option<Box<Type>>,
    },
    /// A subcommand enum's.
    Subcommands {
        /// The subcommand enums of its `#[command(flatten)]` variants, in subcommand order. Their
        /// categories come after the enum's own.
        flattened_subcommand_enums: Vec<Type>,
        /// Its declared categories in declared order, then inherited ones in first-use order.
        /// Headings render in this order.
        sections: Vec<Section>,
    },
}

impl Categories {
    /// The categories of a struct's subcommand enum and flattened `Args` types, with one `Option`
    /// peeled since clap allows either to be optional: `Cmd` for `cmd: Option<Cmd>`.
    pub(crate) fn of_args(fields: &[FieldInput]) -> Self {
        let types_of = |wanted_kind: FieldKind| {
            fields
                .iter()
                .filter(move |field| FieldKind::from(field.attrs.as_slice()) == wanted_kind)
                .map(|field| field.ty.peel_option().clone())
        };
        Self::Args {
            flattened_args: types_of(FieldKind::Flatten).collect(),
            subcommand_enum: types_of(FieldKind::Subcommand).next().map(Box::new),
        }
    }

    /// File each variant's `#[couture(category = "...")]` under the enum's declared categories.
    ///
    /// # Errors
    /// - A category the enum neither declares nor inherits, unless it declares and inherits none.
    /// - A category on a `#[command(flatten)]` variant, which is no command itself.
    /// - A category on a `#[command(skip)]` or `#[command(external_subcommand)]` variant.
    /// - A category declared twice.
    pub(crate) fn parse(
        container: &mut ContainerAttrs,
        variants: &[VariantInput],
        naming: &SubcommandNaming,
    ) -> darling::Result<Self> {
        // Without `inherit`, an enum declaring no categories may name any.
        let inheritance = container.inherit.take().unwrap_or(if container.categories.is_empty() {
            Inheritance::Any
        } else {
            Inheritance::Nothing
        });

        let mut errors = Error::accumulator();

        // The categories the enum declares, in declared order, each once.
        let mut declared: Vec<Category> = Vec::with_capacity(container.categories.len());
        for CategorySpec { description, label, title } in container.categories.drain(..) {
            let value = label.value();
            if declared.iter().any(|category| category.label == value) {
                errors.push(
                    Error::custom(format!("category `{value}` is already declared"))
                        .with_span(&label),
                );
                continue;
            }
            declared.push(Category { description, label: value, title });
        }
        let declared_labels: Vec<&str> =
            declared.iter().map(|category| category.label.as_str()).collect();
        let is_declared = |label: &LitStr| declared_labels.contains(&label.value().as_str());

        // Each categorized variant's label and subcommand name, in subcommand order.
        let categorized_subcommands: Vec<(&LitStr, ClapName)> = variants
            .iter()
            .filter_map(|variant| {
                let label = variant.category.as_ref()?;
                if variant.attrs.has_clap_key("flatten") {
                    errors.push(
                        Error::custom(
                            "`category` on a `#[command(flatten)]` variant files nothing; put it \
                             on the flattened enum's variants",
                        )
                        .with_span(label),
                    );
                    return None;
                }
                if let Some(key) = ["skip", "external_subcommand"]
                    .into_iter()
                    .find(|key| variant.attrs.has_clap_key(key))
                {
                    errors.push(
                        Error::custom(format!(
                            "`category` on a `#[command({key})]` variant files nothing; clap \
                             parses no subcommand of it"
                        ))
                        .with_span(label),
                    );
                    return None;
                }
                Some((label, naming.name_of(variant)))
            })
            .collect();

        let is_first_use = |index: usize, label: &LitStr| {
            categorized_subcommands
                .iter()
                .position(|(other_label, _)| other_label.value() == label.value())
                == Some(index)
        };

        // Each category a subcommand names that the enum doesn't declare, in subcommand order.
        let inherited: Vec<Category> = categorized_subcommands
            .iter()
            .enumerate()
            .filter(|(index, (label, _))| !is_declared(label) && is_first_use(*index, label))
            .filter_map(|(_, (label, _))| {
                errors.handle(inheritance.check(label, &declared_labels)).map(|()| label)
            })
            .map(|label| Category { description: None, label: label.value(), title: None })
            .collect();

        // Ordered sections: the enum's own, then its parent's, each with its subcommands.
        let sections = declared
            .into_iter()
            .chain(inherited)
            .map(|category| {
                let commands = categorized_subcommands
                    .iter()
                    .filter(|(label, _)| label.value() == category.label)
                    .map(|(_, command)| command.clone())
                    .collect();
                Section { category, commands }
            })
            .collect();

        // The subcommand enum of each `#[command(flatten)]` newtype variant; clap rejects any other
        // shape.
        let flattened_subcommand_enums = variants
            .iter()
            .filter(|variant| variant.attrs.has_clap_key("flatten"))
            .filter_map(|variant| variant.fields.fields.first())
            .map(|field| field.ty.clone())
            .collect();

        errors.finish_with(Self::Subcommands { flattened_subcommand_enums, sections })
    }
}

impl ToTokens for Categories {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let categories_of_couture =
            |couture_type: &Type| quote!(<#couture_type as #Runtime::Couture>::CATEGORIES);
        let (sections, flattened_category_maps): (&[Section], Vec<TokenStream2>) = match self {
            Self::Args { flattened_args, subcommand_enum } => (
                &[],
                subcommand_enum
                    .as_deref()
                    .into_iter()
                    .map(categories_of_couture)
                    .chain(flattened_args.iter().map(|args_type| {
                        quote!(<#Runtime::__private::MaybeCouture<#args_type>>::CATEGORIES)
                    }))
                    .collect(),
            ),
            Self::Subcommands { flattened_subcommand_enums, sections } => {
                (sections, flattened_subcommand_enums.iter().map(categories_of_couture).collect())
            }
        };
        let entries = sections.iter().flat_map(|section| {
            let category = &section.category;
            section
                .commands
                .iter()
                .map(move |command| quote!((#Runtime::CommandName::new(#command), #category)))
        });
        let declared_categories = sections.iter().map(|section| &section.category);
        tokens.extend(quote! {
            #Runtime::CommandCategoryMap::new(&[#(#entries),*])
                .with_declared_categories(&[#(#declared_categories),*])
                .with_flattened_maps(&[#(#flattened_category_maps),*])
        });
    }
}

impl ToTokens for Category {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let label = &self.label;
        let title = self.title.as_ref().map(|title| quote!(.with_title(#title)));
        let description =
            self.description.as_ref().map(|description| quote!(.with_description(#description)));
        tokens.extend(quote!(#Runtime::Category::new(#label) #title #description));
    }
}
