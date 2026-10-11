//! [`Backend`] drawn by [cliclack](https://docs.rs/cliclack).

use std::io;

use super::{Backend, CommandStyles, builtin::ClapStyle};
use crate::interactive::{ConfirmPrompt, MultiSelectPrompt, PromptError, SelectPrompt, TextPrompt};

/// Prompts drawn by cliclack, in its clack style.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cliclack;

impl Backend for Cliclack {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Theme::from(prompt.styles).scoped(|| {
            ::cliclack::confirm(prompt.question).initial_value(prompt.default).interact()
        })
    }

    fn multi_select(&self, prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        Theme::from(prompt.styles).scoped(|| {
            prompt
                .options
                .iter()
                .fold(::cliclack::multiselect(prompt.question), |select, option| {
                    let hint = option.get_help().map(ToString::to_string).unwrap_or_default();
                    select.item(option.get_name().to_owned(), option.get_name(), hint)
                })
                .initial_values(prompt.defaults.to_vec())
                // TODO(markovejnovic): cliclack says "Input required" on an empty selection and
                // can't be told to say `prompt.selection_required` instead.
                .required(!prompt.optional)
                .interact()
        })
    }

    fn report(&self, error: &str, styles: CommandStyles<'_>) -> Result<(), PromptError> {
        Theme::from(styles).scoped(|| ::cliclack::log::error(error))
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Theme::from(prompt.styles).scoped(|| {
            let select = prompt.options.iter().fold(
                ::cliclack::select(prompt.question),
                |select, option| {
                    let hint = option.get_help().map(ToString::to_string).unwrap_or_default();
                    select.item(Some(option.get_name().to_owned()), option.get_name(), hint)
                },
            );
            let mut select =
                if prompt.optional { select.item(None, prompt.none_option, "") } else { select };
            match prompt.default {
                Some(default) => select.initial_value(Some(default.to_owned())).interact(),
                None => select.interact(),
            }
        })
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let answer: String = Theme::from(prompt.styles).scoped(|| {
            let input = ::cliclack::input(prompt.question).required(!prompt.optional);
            // TODO(markovejnovic): cliclack's `Input` can't start out holding editable text (no
            // equivalent of inquire's `with_initial_value`), so `initial` is passed as a default
            // instead. The cost: a list prompt pre-filled from the arg's defaults can't be
            // finished early, as Enter on it keeps the default instead of ending the list. Once
            // cliclack supports starting text, pass `initial` that way and show the hint even
            // when there's a default.
            //
            // When there is a default, the hint is left off: cliclack shows the hint where it
            // would show "(default)", hiding it, and Enter wouldn't finish the list anyway.
            let mut input = match (prompt.default.or(prompt.initial), prompt.hint) {
                (Some(default), _) => input.default_input(default),
                (None, Some(hint)) => input.placeholder(hint),
                (None, None) => input,
            };
            input.interact()
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
    /// Run `interact` with this as cliclack's theme.
    fn scoped<T>(self, interact: impl FnOnce() -> io::Result<T>) -> Result<T, PromptError> {
        ::cliclack::set_theme(self);
        let answer = interact();
        ::cliclack::reset_theme();
        answer.map_err(PromptError::from)
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
