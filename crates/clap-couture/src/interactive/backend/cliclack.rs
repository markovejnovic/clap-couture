//! [`Backend`] drawn by [cliclack](https://docs.rs/cliclack).

use std::io;

use super::{Backend, CommandStyles, NONE_OPTION, from_io};
use crate::interactive::{
    ConfirmPrompt, PromptError, SelectPrompt, TextPrompt, console_style::ClapStyle,
};

/// Prompts drawn by cliclack, in its clack style.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cliclack;

impl Backend for Cliclack {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Theme::from(prompt.styles).scoped(prompt.error, || {
            ::cliclack::confirm(prompt.question).initial_value(prompt.default).interact()
        })
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Theme::from(prompt.styles).scoped(prompt.error, || {
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
        let answer: String = Theme::from(prompt.styles).scoped(prompt.error, || {
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
///
/// By default, [`::cliclack`] uses global theming, which is somewhat inconvenient for the use-case
/// of following clap's theme. The [`Theme::scoped`] function allows you to run a function with a
/// temporarily globally set theme.
struct Theme {
    input: console::Style,
    placeholder: console::Style,
}

impl Theme {
    /// Run `interact` with this as cliclack's theme, after logging `error`.
    fn scoped<T>(
        self,
        error: Option<&str>,
        interact: impl FnOnce() -> io::Result<T>,
    ) -> Result<T, PromptError> {
        ::cliclack::set_theme(self);
        let answer = error.map_or(Ok(()), ::cliclack::log::error).and_then(|()| interact());
        ::cliclack::reset_theme();
        answer.map_err(from_io)
    }
}

impl From<CommandStyles<'_>> for Theme {
    fn from(CommandStyles(styles): CommandStyles<'_>) -> Self {
        Self {
            input: console::Style::from(ClapStyle(styles.get_literal())),
            placeholder: console::Style::from(ClapStyle(styles.get_placeholder())),
        }
    }
}

impl ::cliclack::Theme for Theme {
    fn input_style(&self, _state: &::cliclack::ThemeState) -> console::Style {
        self.input.clone()
    }

    fn placeholder_style(&self, _state: &::cliclack::ThemeState) -> console::Style {
        self.placeholder.clone()
    }
}
