//! Ask your own questions after the parse, in the same UI as the prompted args.

use clap::{
    ValueEnum,
    builder::{PossibleValue, Styles},
};

use super::{
    Backend, CommandStyles, ConfirmPrompt, DefaultBackend, MultiSelectPrompt, PromptError,
    SelectPrompt, TextPrompt,
};
use crate::Messages;

/// Asks through a [`Backend`], themed with a command's styles and worded with its [`Messages`].
///
/// Get one from [`CoutureParser::prompter`](crate::CoutureParser::prompter).
#[derive(Debug)]
pub struct Prompter<B = DefaultBackend> {
    backend: B,
    messages: Messages,
    styles: Styles,
}

impl<B> Prompter<B>
where
    B: Backend,
{
    /// Ask a yes/no question, with `default` the answer Enter accepts.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    pub fn confirm(&self, question: &str, default: bool) -> Result<bool, PromptError> {
        self.backend.confirm(&ConfirmPrompt { default, question, styles: self.styles() })
    }

    /// Ask to pick at least one of `T`'s visible values, returned in declared order.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    pub fn multi_select<T>(&self, question: &str) -> Result<Vec<T>, PromptError>
    where
        T: ValueEnum,
    {
        let selection_required = self.messages.selection_required.get();
        let prompt = MultiSelectPrompt {
            defaults: &[],
            optional: false,
            options: &visible_values::<T>(),
            question,
            selection_required: &selection_required,
            styles: self.styles(),
        };
        loop {
            let picked = self.backend.multi_select(&prompt)?;
            match picked.iter().map(|name| value_named(name)).collect() {
                Ok(values) => return Ok(values),
                Err(complaint) => self.backend.report(&complaint, self.styles())?,
            }
        }
    }

    /// `None` when `backend` has no terminal to ask on.
    pub(crate) fn new(backend: B, messages: Messages, styles: Styles) -> Option<Self> {
        backend.is_available().then_some(Self { backend, messages, styles })
    }

    /// Ask to pick one of `T`'s visible values.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    pub fn select<T>(&self, question: &str) -> Result<T, PromptError>
    where
        T: ValueEnum,
    {
        let none_option = self.messages.none.get();
        let prompt = SelectPrompt {
            default: None,
            none_option: &none_option,
            optional: false,
            options: &visible_values::<T>(),
            question,
            styles: self.styles(),
        };
        loop {
            // A required select always picks something; a misbehaving backend's `None` is asked
            // again like any other rejected answer.
            let picked = self.backend.select(&prompt)?.unwrap_or_default();
            match value_named(&picked) {
                Ok(value) => return Ok(value),
                Err(complaint) => self.backend.report(&complaint, self.styles())?,
            }
        }
    }

    const fn styles(&self) -> CommandStyles<'_> {
        CommandStyles(&self.styles)
    }

    /// Ask for free text.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    pub fn text(&self, question: &str) -> Result<String, PromptError> {
        let answer = self.backend.text(&TextPrompt {
            default: None,
            hint: None,
            initial: None,
            optional: false,
            question,
            styles: self.styles(),
        })?;
        Ok(answer.unwrap_or_default())
    }
}

/// The `T` named `name`, or clap's wording for a value that names none.
fn value_named<T>(name: &str) -> Result<T, String>
where
    T: ValueEnum,
{
    T::value_variants()
        .iter()
        .find(|value| value.to_possible_value().is_some_and(|value| value.matches(name, false)))
        .cloned()
        .ok_or_else(|| format!("invalid value '{name}'"))
}

/// `T`'s possible values that are not hidden, in declared order.
fn visible_values<T>() -> Vec<PossibleValue>
where
    T: ValueEnum,
{
    T::value_variants()
        .iter()
        .filter_map(ValueEnum::to_possible_value)
        .filter(|value| !value.is_hide_set())
        .collect()
}
