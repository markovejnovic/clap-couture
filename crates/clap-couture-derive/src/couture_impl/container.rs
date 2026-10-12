//! The deriving type's own `#[couture(...)]`s.
//!
//! `categories = { "key" = { ... } }` has string keys, which are not syn `Meta`, so darling can't
//! read these; they are parsed here instead, with darling's "did you mean" on an unknown key.

use darling::Error;
use syn::{
    Attribute, Expr, Ident, LitBool, LitStr, Path, Token, braced, bracketed,
    ext::IdentExt as _,
    meta::ParseNestedMeta,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use super::messages::Messages;
use crate::{suggest::Suggestion, text::TextExpr};

/// One category of `categories = { ... }`, as written.
pub(crate) struct CategorySpec {
    pub(crate) description: Option<TextExpr>,
    pub(crate) label: LitStr,
    pub(crate) title: Option<TextExpr>,
}

/// Every `#[couture(...)]` on the deriving type, merged.
#[derive(Default)]
pub(crate) struct ContainerAttrs {
    /// Every `categories = { ... }`'s, in declared order.
    pub(crate) categories: Vec<CategorySpec>,
    /// The last `crate = ...`.
    pub(crate) crate_path: Option<Path>,
    /// Where each `categories` and `inherit` was written; only an enum takes them.
    pub(crate) enum_only_keys: Vec<Ident>,
    /// The last `inherit = ...`.
    pub(crate) inherit: Option<Inheritance>,
    /// The last `messages = { ... }`, which wins outright.
    pub(crate) messages: Option<Messages>,
}

/// A `key = expr` inside braces.
pub(crate) struct Entry {
    pub(crate) key: Ident,
    pub(crate) value: Expr,
}

/// Which categories the enum's subcommands may name without the enum declaring them.
#[derive(Clone)]
pub(crate) enum Inheritance {
    /// `inherit = true`, or no `inherit` on an enum that declares no categories.
    Any,
    /// `inherit = false`, or no `inherit` on an enum that declares categories.
    Nothing,
    /// `inherit = ["key", ...]`.
    Only(Vec<String>),
}

/// The keys one level of an attribute takes, for refusing any other with a suggestion.
pub(crate) struct Keys(pub(crate) &'static [&'static str]);

impl ContainerAttrs {
    /// The keys `#[couture(...)]` takes on the deriving type.
    const KEYS: Keys = Keys(&["categories", "crate", "inherit", "messages"]);

    /// Parse and merge each `#[couture(...)]` in `attrs`.
    ///
    /// # Errors
    /// A malformed `#[couture(...)]`, or one with an unknown key.
    pub(crate) fn parse(attrs: &[Attribute]) -> darling::Result<Self> {
        let mut parsed = Self::default();
        let mut errors = Error::accumulator();
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("couture")) {
            errors.handle(
                attr.parse_nested_meta(|meta| parsed.parse_key(&meta)).map_err(Error::from),
            );
        }
        errors.finish_with(parsed)
    }

    fn parse_key(&mut self, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
        let key = meta.path.get_ident().ok_or_else(|| meta.error("expected a key"))?;
        match key.to_string().as_str() {
            "categories" => {
                let value = meta.value()?;
                let content;
                braced!(content in value);
                self.categories
                    .extend(Punctuated::<CategorySpec, Token![,]>::parse_terminated(&content)?);
                self.enum_only_keys.push(key.clone());
            }
            "crate" => {
                let value = meta.value()?;
                self.crate_path = Some(if value.peek(LitStr) {
                    value.parse::<LitStr>()?.parse_with(Path::parse_mod_style)?
                } else {
                    value.call(Path::parse_mod_style)?
                });
            }
            "inherit" => {
                self.inherit = Some(meta.value()?.parse()?);
                self.enum_only_keys.push(key.clone());
            }
            "messages" => {
                let value = meta.value()?;
                let content;
                braced!(content in value);
                self.messages = Some(content.parse()?);
            }
            _ => return Err(Self::KEYS.unknown(key)),
        }
        Ok(())
    }

    /// Refuse the keys only an enum takes, for a struct.
    ///
    /// # Errors
    /// A `categories` or `inherit`, which a struct has no subcommands to apply to.
    pub(crate) fn refuse_enum_only_keys(&self) -> darling::Result<()> {
        let mut errors = Error::accumulator();
        for key in &self.enum_only_keys {
            errors.push(
                Error::custom(format!(
                    "`{key}` applies to a subcommand enum; put it on the enum that derives \
                     `Subcommand`"
                ))
                .with_span(key),
            );
        }
        errors.finish()
    }
}

impl Parse for CategorySpec {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        const KEYS: Keys = Keys(&["description", "title"]);
        let label = input.parse()?;
        input.parse::<Token![=]>()?;
        let content;
        braced!(content in input);
        let (mut description, mut title) = (None, None);
        for Entry { key, value } in Punctuated::<Entry, Token![,]>::parse_terminated(&content)? {
            KEYS.check(&key)?;
            let text = Some(TextExpr::from(value));
            if key == "title" {
                title = text;
            } else {
                description = text;
            }
        }
        Ok(Self { description, label, title })
    }
}

impl Parse for Entry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key = input.call(Ident::parse_any)?;
        input.parse::<Token![=]>()?;
        Ok(Self { key, value: input.parse()? })
    }
}

impl Inheritance {
    /// Check that a subcommand may name `category` without the enum declaring it, suggesting
    /// the closest of `declared_labels` and the inherited labels when it may not.
    pub(crate) fn check(&self, category: &LitStr, declared_labels: &[&str]) -> darling::Result<()> {
        let label = category.value();
        let inherited_labels = match self {
            Self::Any => return Ok(()),
            Self::Nothing => &[][..],
            Self::Only(labels) if labels.contains(&label) => return Ok(()),
            Self::Only(labels) => labels.as_slice(),
        };
        let known =
            declared_labels.iter().copied().chain(inherited_labels.iter().map(String::as_str));
        let suggestion = Suggestion::closest(&label, known)
            .map(|suggestion| format!("; {suggestion}"))
            .unwrap_or_default();
        Err(Error::custom(format!(
            "category `{label}` is not declared or inherited in this enum's \
             `#[couture(...)]`{suggestion}"
        ))
        .with_span(category))
    }
}

impl Parse for Inheritance {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitBool) {
            let inherits = input.parse::<LitBool>()?.value;
            return Ok(if inherits { Self::Any } else { Self::Nothing });
        }
        if !input.peek(syn::token::Bracket) {
            return Err(input.error("expected `true`, `false` or a list of category keys"));
        }
        let content;
        bracketed!(content in input);
        let labels = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
        Ok(Self::Only(labels.iter().map(LitStr::value).collect()))
    }
}

impl Keys {
    /// Refuse `key` unless it is one of these.
    pub(crate) fn check(&self, key: &Ident) -> syn::Result<()> {
        if self.0.iter().any(|known| key == known) {
            return Ok(());
        }
        Err(self.unknown(key))
    }

    /// The error for `key`, suggesting the closest of these.
    fn unknown(&self, key: &Ident) -> syn::Error {
        let error = Error::unknown_field_with_alts(&key.unraw().to_string(), self.0.iter());
        syn::Error::new(key.span(), error.to_string())
    }
}
