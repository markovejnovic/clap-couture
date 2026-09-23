//! [`Prompter`] drawn by [inquire](https://docs.rs/inquire).

use std::io::{self, Write as _};

use clap::builder::{
    PossibleValue, Styles,
    styling::{AnsiColor, Color, Effects, Style},
};
use inquire::{
    Confirm, InquireError, Select, Text,
    ui::{Attributes, ErrorMessageRenderConfig, RenderConfig, StyleSheet},
    validator::ValueRequiredValidator,
};

use super::{ConfirmPrompt, NONE_OPTION, PromptError, Prompter, SelectPrompt, TextPrompt, from_io};

/// Prompts drawn by inquire.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inquire;

impl Prompter for Inquire {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        report(prompt.error)?;
        Confirm::new(prompt.question)
            .with_default(prompt.default)
            .with_render_config(render_config(prompt.styles))
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
        let select =
            Select::new(prompt.question, items).with_render_config(render_config(prompt.styles));
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
        let text = Text::new(prompt.question).with_render_config(render_config(prompt.styles));
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

const fn named(ansi: AnsiColor) -> inquire::ui::Color {
    match ansi {
        AnsiColor::Black => inquire::ui::Color::Black,
        AnsiColor::Red => inquire::ui::Color::DarkRed,
        AnsiColor::Green => inquire::ui::Color::DarkGreen,
        AnsiColor::Yellow => inquire::ui::Color::DarkYellow,
        AnsiColor::Blue => inquire::ui::Color::DarkBlue,
        AnsiColor::Magenta => inquire::ui::Color::DarkMagenta,
        AnsiColor::Cyan => inquire::ui::Color::DarkCyan,
        AnsiColor::White => inquire::ui::Color::Grey,
        AnsiColor::BrightBlack => inquire::ui::Color::DarkGrey,
        AnsiColor::BrightRed => inquire::ui::Color::LightRed,
        AnsiColor::BrightGreen => inquire::ui::Color::LightGreen,
        AnsiColor::BrightYellow => inquire::ui::Color::LightYellow,
        AnsiColor::BrightBlue => inquire::ui::Color::LightBlue,
        AnsiColor::BrightMagenta => inquire::ui::Color::LightMagenta,
        AnsiColor::BrightCyan => inquire::ui::Color::LightCyan,
        AnsiColor::BrightWhite => inquire::ui::Color::White,
    }
}

fn render_config(styles: &Styles) -> RenderConfig<'static> {
    let base = RenderConfig::default_colored();
    RenderConfig {
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

/// Show clap's complaint about the previous answer, if any.
fn report(error: Option<&str>) -> Result<(), PromptError> {
    let Some(error) = error else { return Ok(()) };
    writeln!(io::stderr(), "{error}").map_err(PromptError::Io)
}

/// `style`'s foreground color and its bold and italic effects.
fn sheet(style: &Style) -> StyleSheet {
    let color = style.get_fg_color().map(|color| match color {
        Color::Ansi(ansi) => named(ansi),
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
