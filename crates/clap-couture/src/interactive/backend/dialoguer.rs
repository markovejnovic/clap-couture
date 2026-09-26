//! [`Backend`] drawn by [dialoguer](https://docs.rs/dialoguer).

use clap::builder::PossibleValue;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use super::{Backend, ClapStyle, CommandStyles, NONE_OPTION};
use crate::interactive::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

/// A dialoguer failure.
struct DialoguerError(dialoguer::Error);

/// Prompts drawn by dialoguer's colorful theme.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dialoguer;

impl Backend for Dialoguer {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        let theme = ColorfulTheme::from(prompt.styles);
        Confirm::with_theme(&theme)
            .with_prompt(prompt.question)
            .default(prompt.default)
            .interact_opt()
            .map_err(DialoguerError)?
            .ok_or(PromptError::Cancelled)
    }

    fn report(&self, error: &str, styles: CommandStyles<'_>) -> Result<(), PromptError> {
        let theme = ColorfulTheme::from(styles);
        console::Term::stderr()
            .write_line(&theme.error_style.apply_to(error).to_string())
            .map_err(PromptError::Io)
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        let theme = ColorfulTheme::from(prompt.styles);
        let names: Vec<&str> = prompt.options.iter().map(PossibleValue::get_name).collect();
        let none = prompt.optional.then_some(NONE_OPTION);
        let items: Vec<&str> = names.iter().copied().chain(none).collect();
        let select = Select::with_theme(&theme).with_prompt(prompt.question).items(&items);
        let default =
            prompt.default.and_then(|default| names.iter().position(|name| *name == default));
        let picked = match default {
            Some(index) => select.default(index),
            None => select,
        }
        .interact_opt()
        .map_err(DialoguerError)?
        .ok_or(PromptError::Cancelled)?;
        // Past the options is the `NONE_OPTION` item, only listed for an optional prompt.
        Ok(names.get(picked).map(|name| (*name).to_owned()))
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let theme = ColorfulTheme::from(prompt.styles);
        let input = Input::<String>::with_theme(&theme)
            .with_prompt(prompt.question)
            .allow_empty(prompt.optional);
        let answer = match prompt.default {
            Some(default) => input.default(default.to_owned()),
            None => input,
        }
        .interact_text()
        .map_err(DialoguerError)?;
        Ok(Some(answer).filter(|answer| !answer.is_empty()))
    }
}

impl From<DialoguerError> for PromptError {
    fn from(DialoguerError(err): DialoguerError) -> Self {
        let dialoguer::Error::IO(err) = err;
        Self::from(err)
    }
}

impl From<CommandStyles<'_>> for ColorfulTheme {
    fn from(CommandStyles(styles): CommandStyles<'_>) -> Self {
        Self {
            defaults_style: console::Style::from(ClapStyle(styles.get_placeholder())),
            error_style: console::Style::from(ClapStyle(styles.get_error())),
            prompt_style: console::Style::from(ClapStyle(styles.get_header())),
            values_style: console::Style::from(ClapStyle(styles.get_literal())),
            ..Self::default()
        }
    }
}
