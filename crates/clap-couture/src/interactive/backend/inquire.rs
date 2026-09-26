//! [`Backend`] drawn by [inquire](https://docs.rs/inquire).

use std::io;

use clap::builder::{
    PossibleValue,
    styling::{AnsiColor, Color, Effects},
};
use inquire::{
    Confirm, Select, Text,
    ui::{Attributes, ErrorMessageRenderConfig, RenderConfig, StyleSheet},
    validator::ValueRequiredValidator,
};

use super::{Backend, ClapColor, ClapStyle, CommandStyles, NONE_OPTION, NamedColor, from_io};
use crate::interactive::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

struct InquireError(inquire::InquireError);

/// Prompts drawn by inquire.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inquire;

impl Backend for Inquire {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Ok(Confirm::new(prompt.question)
            .with_default(prompt.default)
            .with_render_config(RenderConfig::from(prompt.styles))
            .prompt()
            .map_err(InquireError)?)
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
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
        .map_err(InquireError)?;
        // Past the options is the `NONE_OPTION` item, only listed for an optional prompt.
        Ok(names.get(picked.index).map(|name| (*name).to_owned()))
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
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
        let answer = text.prompt().map_err(InquireError)?;
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
            answer: StyleSheet::from(ClapStyle(styles.get_literal())),
            default_value: StyleSheet::from(ClapStyle(styles.get_placeholder())),
            error_message: ErrorMessageRenderConfig {
                message: StyleSheet::from(ClapStyle(styles.get_error())),
                ..base.error_message
            },
            prompt: StyleSheet::from(ClapStyle(styles.get_header())),
            ..base
        }
    }
}

impl From<ClapColor> for inquire::ui::Color {
    fn from(ClapColor(color): ClapColor) -> Self {
        match color {
            Color::Ansi(ansi) => Self::from(NamedColor(ansi)),
            Color::Ansi256(index) => Self::AnsiValue(index.0),
            Color::Rgb(rgb) => Self::Rgb { r: rgb.0, g: rgb.1, b: rgb.2 },
        }
    }
}

impl From<ClapStyle<'_>> for StyleSheet {
    fn from(ClapStyle(style): ClapStyle<'_>) -> Self {
        let base = style
            .get_fg_color()
            .map_or_else(Self::new, |color| Self::new().with_fg(ClapColor(color).into()));
        [(Effects::BOLD, Attributes::BOLD), (Effects::ITALIC, Attributes::ITALIC)]
            .into_iter()
            .filter(|(effect, _)| style.get_effects().contains(*effect))
            .fold(base, |sheet, (_, attribute)| sheet.with_attr(attribute))
    }
}

impl From<InquireError> for PromptError {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "`InquireError` is #[non_exhaustive]; anything unlisted is an I/O-level failure"
    )]
    fn from(InquireError(err): InquireError) -> Self {
        match err {
            inquire::InquireError::IO(err) => from_io(err),
            inquire::InquireError::NotTTY => Self::NotATerminal,
            inquire::InquireError::OperationCanceled
            | inquire::InquireError::OperationInterrupted => Self::Cancelled,
            other => Self::Io(io::Error::other(other)),
        }
    }
}
