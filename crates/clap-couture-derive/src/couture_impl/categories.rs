//! `CATEGORIES`: the category each subcommand is filed under.

use core::fmt;

use darling::Error;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use serde::{
    Deserialize, Deserializer,
    de::{SeqAccess, Visitor, value::SeqAccessDeserializer},
};
use serde_tokenstream::OrderedMap;
use syn::{Attribute, LitStr, Type};

use super::messages::Messages;
use crate::{
    clap_compat::{ClapAttrs as _, ClapFieldType as _, FieldKind, SubcommandNaming},
    input::{FieldInput, VariantInput},
    text::{OptionalText, TextExpr},
};

/// One category, as `clap_couture::Category`.
struct Category {
    description: Option<TextExpr>,
    label: String,
    title: Option<TextExpr>,
}

/// A variant's `#[category("...")]`: the label it files the variant under, kept as a literal for
/// its span. A variant with several uses the last.
struct CategoryAttr(LitStr);

/// A category, and the subcommands filed under it.
pub(crate) struct Section {
    category: Category,
    commands: Vec<String>,
}

/// One `#[couture(...)]` on the deriving type.
#[derive(Deserialize)]
pub(crate) struct Spec {
    /// In declared order.
    #[serde(default, deserialize_with = "in_declared_order")]
    categories: Vec<(String, SpecCategory)>,
    #[serde(default)]
    inherit: Option<Inheritance>,
    #[serde(default)]
    pub(crate) messages: Option<Messages>,
}

impl TryFrom<&Attribute> for CategoryAttr {
    type Error = Error;

    fn try_from(attr: &Attribute) -> darling::Result<Self> {
        Ok(Self(attr.parse_args()?))
    }
}

impl Spec {
    fn parse(attr: &Attribute) -> darling::Result<Self> {
        let list = attr.meta.require_list()?;
        Ok(serde_tokenstream::from_tokenstream_spanned(list.delimiter.span(), &list.tokens)?)
    }

    /// Each `#[couture(...)]` in `attrs`, parsed.
    ///
    /// # Errors
    /// - A malformed `#[couture(...)]`.
    pub(crate) fn parse_all(attrs: &[Attribute]) -> darling::Result<Vec<Self>> {
        attrs.iter().filter(|attr| attr.path().is_ident("couture")).map(Self::parse).collect()
    }
}

#[derive(Deserialize)]
struct SpecCategory {
    description: Option<TextExpr>,
    title: Option<TextExpr>,
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

    /// File each variant's `#[category("...")]` under the categories of the enum's `specs`.
    ///
    /// # Errors
    /// - A malformed `#[category(...)]`.
    /// - A category the enum neither declares nor inherits, unless it declares and inherits none.
    /// - A `#[category("...")]` on a `#[command(flatten)]` variant, which is no command itself.
    pub(crate) fn parse(
        specs: Vec<Spec>,
        variants: &[VariantInput],
        naming: &SubcommandNaming,
    ) -> darling::Result<Self> {
        let inheritance = Inheritance::new(&specs);

        // The categories the enum declares, in declared order.
        let declared: Vec<Category> = specs
            .into_iter()
            .flat_map(|spec| spec.categories)
            .map(|(label, spec_category)| Category {
                description: spec_category.description,
                label,
                title: spec_category.title,
            })
            .collect();
        let is_declared =
            |label: &LitStr| declared.iter().any(|category| category.label == label.value());

        let mut errors = Error::accumulator();

        // Each categorized variant's `#[category("...")]` and subcommand name, in subcommand order.
        let categorized_subcommands: Vec<(LitStr, String)> = variants
            .iter()
            .filter_map(|variant| {
                let last_category_attr = variant
                    .attrs
                    .iter()
                    .rev()
                    .find(|attr| attr.path().is_ident("category"))
                    .map(CategoryAttr::try_from)?;
                let CategoryAttr(label) = errors.handle(last_category_attr)?;
                if variant.attrs.has_clap_key("flatten") {
                    errors.push(
                        Error::custom(
                            "`#[category]` on a `#[command(flatten)]` variant files nothing; put \
                             it on the flattened enum's variants",
                        )
                        .with_span(&label),
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
            .filter_map(|(_, (label, _))| errors.handle(inheritance.check(label)).map(|()| label))
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

/// Which categories the enum's subcommands may name without the enum declaring them.
#[derive(Clone)]
enum Inheritance {
    /// `inherit = true`, or no `inherit` on an enum that declares no categories.
    Any,
    /// `inherit = false`, or no `inherit` on an enum that declares categories.
    Nothing,
    /// `inherit = ["key", ...]`.
    Only(Vec<String>),
}

impl Inheritance {
    /// Check that a subcommand may name `category` without the enum declaring it.
    fn check(&self, category: &LitStr) -> darling::Result<()> {
        let label = category.value();
        let allowed = match self {
            Self::Any => true,
            Self::Nothing => false,
            Self::Only(labels) => labels.contains(&label),
        };
        if allowed {
            return Ok(());
        }
        Err(Error::custom(format!(
            "category `{label}` is not declared or inherited in this enum's `#[couture(...)]`"
        ))
        .with_span(category))
    }

    /// The enum's last `inherit = ...`; without one, `Any` if the enum declares no categories,
    /// else `Nothing`.
    fn new(specs: &[Spec]) -> Self {
        specs.iter().rev().find_map(|spec| spec.inherit.clone()).unwrap_or_else(|| {
            if specs.iter().all(|spec| spec.categories.is_empty()) {
                Self::Any
            } else {
                Self::Nothing
            }
        })
    }
}

impl<'de> Deserialize<'de> for Inheritance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(InheritanceVisitor)
    }
}

/// Reads `inherit = true`, `inherit = false` or `inherit = ["key", ...]` as an [`Inheritance`].
struct InheritanceVisitor;

impl<'de> Visitor<'de> for InheritanceVisitor {
    type Value = Inheritance;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("`true`, `false` or a list of category keys")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Inheritance, E> {
        Ok(if v { Inheritance::Any } else { Inheritance::Nothing })
    }

    fn visit_seq<A>(self, seq: A) -> Result<Inheritance, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Vec::deserialize(SeqAccessDeserializer::new(seq)).map(Inheritance::Only)
    }
}

impl ToTokens for Categories {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let categories_of_couture =
            |couture_type: &Type| quote!(<#couture_type as ::clap_couture::Couture>::CATEGORIES);
        let (sections, flattened_category_maps): (&[Section], Vec<TokenStream2>) = match self {
            Self::Args { flattened_args, subcommand_enum } => (
                &[],
                subcommand_enum
                    .as_deref()
                    .into_iter()
                    .map(categories_of_couture)
                    .chain(flattened_args.iter().map(
                        |args_type| quote!(<::clap_couture::MaybeCouture<#args_type>>::CATEGORIES),
                    ))
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
                .map(move |command| quote!((::clap_couture::CommandName::new(#command), #category)))
        });
        let declared_categories = sections.iter().map(|section| &section.category);
        tokens.extend(quote! {
            ::clap_couture::CommandCategoryMap::new(&[#(#entries),*])
                .with_declared_categories(&[#(#declared_categories),*])
                .with_flattened_maps(&[#(#flattened_category_maps),*])
        });
    }
}

impl ToTokens for Category {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let label = &self.label;
        let description = OptionalText::from(&self.description);
        let title = OptionalText::from(&self.title);
        tokens.extend(quote! {
            ::clap_couture::Category { description: #description, label: #label, title: #title }
        });
    }
}

/// Deserialize `categories = { ... }` as pairs, keeping the order they are written in.
fn in_declared_order<'de, D>(deserializer: D) -> Result<Vec<(String, SpecCategory)>, D::Error>
where
    D: Deserializer<'de>,
{
    OrderedMap::deserialize(deserializer).map(|categories| categories.into_iter().collect())
}
