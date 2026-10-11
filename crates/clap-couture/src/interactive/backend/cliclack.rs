//! [`Backend`] drawn by [cliclack](https://docs.rs/cliclack).

use std::io;

#[cfg(feature = "interactive-editor")]
use super::builtin::Edited;
use super::{Backend, CommandStyles, builtin::ClapStyle};
#[cfg(feature = "interactive-editor")]
use crate::interactive::EditorPrompt;
use crate::interactive::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

/// Prompts drawn by cliclack, in its clack style.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cliclack;

impl Backend for Cliclack {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Theme::from(prompt.styles).scoped(|| {
            ::cliclack::confirm(prompt.question).initial_value(prompt.default).interact()
        })
    }

    /// The waiting hint sits in cliclack's gutter, and the closed editor leaves the question
    /// with the answer's first line, as a submitted input does.
    #[cfg(feature = "interactive-editor")]
    fn editor(&self, prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
        use ::cliclack::{Theme as _, ThemeState};

        let theme = Theme::from(prompt.styles);
        // cliclack's gutter bar, `│`, as its footer draws it under a submitted prompt.
        let footer = theme.format_footer(&ThemeState::Submit);
        let bar = footer.trim_end();
        let waiting = if prompt.waiting.is_empty() {
            String::new()
        } else {
            format!("{bar}  {}", prompt.waiting)
        };
        let hinted = EditorPrompt { waiting: &waiting, ..*prompt };
        let answer = super::EditorInteraction::start(self, &hinted)
            .and_then(super::EditorInteraction::answer);
        let transcript = match Edited::of(&answer, prompt.optional) {
            Edited::Answered(line) => {
                let input = if line.is_empty() {
                    String::new()
                } else {
                    let line = theme.input_style(&ThemeState::Submit).apply_to(line);
                    format!("{bar}  {line}\n")
                };
                let header = theme.format_header(&ThemeState::Submit, prompt.question);
                format!("{header}{input}{footer}")
            }
            Edited::Cancelled => {
                format!("{}{footer}", theme.format_header(&ThemeState::Cancel, prompt.question))
            }
            Edited::Failed => return answer,
        };
        // Best effort: after a hang-up there is no terminal left to draw on.
        drop(console::Term::stderr().write_str(&transcript));
        answer
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
