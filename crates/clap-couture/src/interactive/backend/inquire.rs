//! [`Backend`] drawn by [inquire](https://docs.rs/inquire).

use std::io::{self, Write as _};

use clap::builder::{
    PossibleValue,
    styling::{AnsiColor, Color, Effects, Style},
};
use inquire::{
    Confirm, InquireError, Select, Text,
    ui::{Attributes, ErrorMessageRenderConfig, RenderConfig, StyleSheet},
    validator::ValueRequiredValidator,
};

use super::{Backend, CommandStyles, NONE_OPTION, NamedColor, from_io};
use crate::interactive::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

/// Prompts drawn by inquire.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inquire;

impl Backend for Inquire {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        report(prompt.error)?;
        Confirm::new(prompt.question)
            .with_default(prompt.default)
            .with_render_config(RenderConfig::from(prompt.styles))
            .prompt()
            .map_err(from_inquire)
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        report(prompt.error)?;
        let names: Vec<&str> = prompt.options.iter().map(PossibleValue::get_name).collect();
        let default =
            prompt.default.and_then(|default| names.iter().position(|name| *name == default));
        let none = prompt.optional.then_some(NONE_OPTION);
        let items: Vec<&str> = names.iter().copied().chain(none).collect();
        let select = Select::new(prompt.question, items)
            .with_render_config(RenderConfig::from(prompt.styles));
        let picked = match default {
            Some(index) => select.with_starting_cursor(index),
            None => select,
        }
        .raw_prompt()
        .map_err(from_inquire)?;
        // Past the options is the `NONE_OPTION` item, only listed for an optional prompt.
        Ok(names.get(picked.index).map(|name| (*name).to_owned()))
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        report(prompt.error)?;
        let text = Text::new(prompt.question).with_render_config(RenderConfig::from(prompt.styles));
        let text = match prompt.default {
            Some(default) => text.with_default(default),
            None => text,
        };
        let text = if prompt.optional {
            text
        } else {
            text.with_validator(ValueRequiredValidator::default())
        };
        let answer = text.prompt().map_err(from_inquire)?;
        Ok(Some(answer).filter(|answer| !answer.is_empty()))
    }
}

impl From<NamedColor> for inquire::ui::Color {
    fn from(NamedColor(ansi): NamedColor) -> Self {
        match ansi {
            AnsiColor::Black => Self::Black,
            AnsiColor::Red => Self::DarkRed,
            AnsiColor::Green => Self::DarkGreen,
            AnsiColor::Yellow => Self::DarkYellow,
            AnsiColor::Blue => Self::DarkBlue,
            AnsiColor::Magenta => Self::DarkMagenta,
            AnsiColor::Cyan => Self::DarkCyan,
            AnsiColor::White => Self::Grey,
            AnsiColor::BrightBlack => Self::DarkGrey,
            AnsiColor::BrightRed => Self::LightRed,
            AnsiColor::BrightGreen => Self::LightGreen,
            AnsiColor::BrightYellow => Self::LightYellow,
            AnsiColor::BrightBlue => Self::LightBlue,
            AnsiColor::BrightMagenta => Self::LightMagenta,
            AnsiColor::BrightCyan => Self::LightCyan,
            AnsiColor::BrightWhite => Self::White,
        }
    }
}

impl From<CommandStyles<'_>> for RenderConfig<'static> {
    fn from(CommandStyles(styles): CommandStyles<'_>) -> Self {
        let base = Self::default_colored();
        Self {
            answer: sheet(styles.get_literal()),
            default_value: sheet(styles.get_placeholder()),
            error_message: ErrorMessageRenderConfig {
                message: sheet(styles.get_error()),
                ..base.error_message
            },
            prompt: sheet(styles.get_header()),
            ..base
        }
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "`InquireError` is #[non_exhaustive]; anything unlisted is an I/O-level failure"
)]
fn from_inquire(err: InquireError) -> PromptError {
    match err {
        InquireError::IO(err) => from_io(err),
        InquireError::NotTTY => PromptError::NotATerminal,
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            PromptError::Cancelled
        }
        other => PromptError::Io(io::Error::other(other)),
    }
}

/// Show clap's complaint about the previous answer, if any.
fn report(error: Option<&str>) -> Result<(), PromptError> {
    let Some(error) = error else { return Ok(()) };
    writeln!(io::stderr(), "{error}").map_err(PromptError::Io)
}

/// `style`'s foreground color and its bold and italic effects.
fn sheet(style: &Style) -> StyleSheet {
    let color = style.get_fg_color().map(|color| match color {
        Color::Ansi(ansi) => inquire::ui::Color::from(NamedColor(ansi)),
        Color::Ansi256(index) => inquire::ui::Color::AnsiValue(index.0),
        Color::Rgb(rgb) => inquire::ui::Color::Rgb { r: rgb.0, g: rgb.1, b: rgb.2 },
    });
    let sheet = color.map_or_else(StyleSheet::new, |color| StyleSheet::new().with_fg(color));
    let effects = style.get_effects();
    [(Effects::BOLD, Attributes::BOLD), (Effects::ITALIC, Attributes::ITALIC)]
        .into_iter()
        .filter(|(effect, _)| effects.contains(*effect))
        .fold(sheet, |sheet, (_, attribute)| sheet.with_attr(attribute))
}
