//! [`Prompter`] drawn by [cliclack](https://docs.rs/cliclack).

use std::io;

use clap::builder::Styles;

use super::{
    ConfirmPrompt, NONE_OPTION, PromptError, Prompter, SelectPrompt, TextPrompt,
    console_style::console_style, from_io,
};

/// Prompts drawn by cliclack, in its clack style.
///
/// cliclack's theme is process-wide: each prompt sets one from the command's styles and restores
/// cliclack's default afterwards.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cliclack;

impl Prompter for Cliclack {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        themed(prompt.styles, prompt.error, || {
            ::cliclack::confirm(prompt.question).initial_value(prompt.default).interact()
        })
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        themed(prompt.styles, prompt.error, || {
            let select = prompt.options.iter().fold(
                ::cliclack::select(prompt.question),
                |select, option| {
                    let hint = option.get_help().map(ToString::to_string).unwrap_or_default();
                    select.item(Some(option.get_name().to_owned()), option.get_name(), hint)
                },
            );
            let mut select =
                if prompt.optional { select.item(None, NONE_OPTION, "") } else { select };
            match prompt.default {
                Some(default) => select.initial_value(Some(default.to_owned())).interact(),
                None => select.interact(),
            }
        })
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let answer: String = themed(prompt.styles, prompt.error, || {
            let mut input = ::cliclack::input(prompt.question).required(!prompt.optional);
            match prompt.default {
                Some(default) => input.default_input(default).interact(),
                None => input.interact(),
            }
        })?;
        Ok(Some(answer).filter(|answer| !answer.is_empty()))
    }
}

/// The command's styles on cliclack's input line.
struct Theme {
    input: console::Style,
    placeholder: console::Style,
}

impl ::cliclack::Theme for Theme {
    fn input_style(&self, _state: &::cliclack::ThemeState) -> console::Style {
        self.input.clone()
    }

    fn placeholder_style(&self, _state: &::cliclack::ThemeState) -> console::Style {
        self.placeholder.clone()
    }
}

/// Run `ask` under a theme taken from `styles`, after logging `error`.
fn themed<T>(
    styles: &Styles,
    error: Option<&str>,
    ask: impl FnOnce() -> io::Result<T>,
) -> Result<T, PromptError> {
    ::cliclack::set_theme(Theme {
        input: console_style(styles.get_literal()),
        placeholder: console_style(styles.get_placeholder()),
    });
    let answer = error.map_or(Ok(()), ::cliclack::log::error).and_then(|()| ask());
    ::cliclack::reset_theme();
    answer.map_err(from_io)
}
