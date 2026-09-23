//! [`Prompter`] drawn by [dialoguer](https://docs.rs/dialoguer).

use std::io;

use clap::builder::{PossibleValue, Styles};
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use super::{
    ConfirmPrompt, PromptError, Prompter, SelectPrompt, TextPrompt, console_style::console_style,
    from_io,
};

/// Prompts drawn by dialoguer's colorful theme.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dialoguer;

impl Prompter for Dialoguer {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        let theme = theme(prompt.styles);
        report(&theme, prompt.error)?;
        Confirm::with_theme(&theme)
            .with_prompt(prompt.question)
            .default(prompt.default)
            .interact_opt()
            .map_err(from_dialoguer)?
            .ok_or(PromptError::Cancelled)
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<String, PromptError> {
        let theme = theme(prompt.styles);
        report(&theme, prompt.error)?;
        let names: Vec<&str> = prompt.options.iter().map(PossibleValue::get_name).collect();
        let select = Select::with_theme(&theme).with_prompt(prompt.question).items(&names);
        let default =
            prompt.default.and_then(|default| names.iter().position(|name| *name == default));
        let picked = match default {
            Some(index) => select.default(index),
            None => select,
        }
        .interact_opt()
        .map_err(from_dialoguer)?
        .ok_or(PromptError::Cancelled)?;
        names
            .get(picked)
            .map(|name| (*name).to_owned())
            .ok_or_else(|| PromptError::Io(io::Error::other("dialoguer picked no listed option")))
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let theme = theme(prompt.styles);
        report(&theme, prompt.error)?;
        let input = Input::<String>::with_theme(&theme)
            .with_prompt(prompt.question)
            .allow_empty(prompt.optional);
        let answer = match prompt.default {
            Some(default) => input.default(default.to_owned()),
            None => input,
        }
        .interact_text()
        .map_err(from_dialoguer)?;
        Ok(Some(answer).filter(|answer| !answer.is_empty()))
    }
}

fn from_dialoguer(err: dialoguer::Error) -> PromptError {
    let dialoguer::Error::IO(err) = err;
    from_io(err)
}

/// Show clap's complaint about the previous answer, if any.
fn report(theme: &ColorfulTheme, error: Option<&str>) -> Result<(), PromptError> {
    let Some(error) = error else { return Ok(()) };
    console::Term::stderr()
        .write_line(&theme.error_style.apply_to(error).to_string())
        .map_err(PromptError::Io)
}

fn theme(styles: &Styles) -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: console_style(styles.get_placeholder()),
        error_style: console_style(styles.get_error()),
        prompt_style: console_style(styles.get_header()),
        values_style: console_style(styles.get_literal()),
        ..ColorfulTheme::default()
    }
}
