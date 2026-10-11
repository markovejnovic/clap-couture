//! [`Backend`] drawn by [cliclack](https://docs.rs/cliclack).

use std::io;

use secrecy::SecretString;

#[cfg(feature = "interactive-editor")]
use super::builtin::Edited;
use super::{Backend, CommandStyles, builtin::ClapStyle};
#[cfg(feature = "interactive-editor")]
use crate::interactive::EditorPrompt;
use crate::interactive::{
    ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt, TextPrompt,
};

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

    fn secret(&self, prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
        let answer = Theme::from(prompt.styles).scoped(|| {
            let input = ::cliclack::password(prompt.question);
            let mut input = if prompt.optional { input.allow_empty() } else { input };
            input.interact()
        })?;
        Ok((!answer.is_empty()).then(|| SecretString::from(answer)))
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

    /// cliclack can't pre-fill its input, so `initial` shows as the placeholder instead: typing
    /// it keeps it, and an empty answer stays empty.
    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let theme = Theme::from(prompt.styles);
        let theme = match prompt.initial {
            Some(_) => theme.with_blank_answer(prompt.hint.unwrap_or_default()),
            None => theme,
        };
        let answer: String = theme.scoped(|| {
            let input = ::cliclack::input(prompt.question).required(!prompt.optional);
            // When there is a default, the hint is left off: cliclack shows the hint where it
            // would show "(default)", hiding it.
            let mut input = match (prompt.default, prompt.initial, prompt.hint) {
                (Some(default), ..) => input.default_input(default),
                (None, Some(initial), Some(hint)) => {
                    input.placeholder(&format!("{initial} {hint}"))
                }
                (None, Some(placeholder), None) | (None, None, Some(placeholder)) => {
                    input.placeholder(placeholder)
                }
                (None, None, None) => input,
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
    /// What an empty answer leaves in the transcript, if not the placeholder.
    blank_answer: Option<String>,
    styles: Styles,
}

impl Theme {
    /// Run `interact` with this as cliclack's theme.
    fn scoped<T>(self, interact: impl FnOnce() -> io::Result<T>) -> Result<T, PromptError> {
        ::cliclack::set_theme(self);
        let answer = interact();
        ::cliclack::reset_theme();
        answer.map_err(PromptError::from)
    }

    /// This theme, leaving `blank_answer` in the transcript for an empty answer.
    fn with_blank_answer(self, blank_answer: &str) -> Self {
        Self { blank_answer: Some(blank_answer.to_owned()), ..self }
    }
}

impl From<CommandStyles<'_>> for Theme {
    fn from(CommandStyles(styles): CommandStyles<'_>) -> Self {
        Self {
            blank_answer: None,
            styles: Styles {
                input: console::Style::from(ClapStyle(styles.get_literal())),
                placeholder: console::Style::from(ClapStyle(styles.get_placeholder())),
            },
        }
    }
}

impl ::cliclack::Theme for Theme {
    fn format_placeholder(
        &self,
        state: &::cliclack::ThemeState,
        cursor: &::cliclack::StringCursor,
    ) -> String {
        match (state, &self.blank_answer) {
            (::cliclack::ThemeState::Submit, Some(blank_answer)) => {
                let mut blank_cursor = ::cliclack::StringCursor::default();
                blank_cursor.extend(blank_answer);
                self.styles.format_placeholder(state, &blank_cursor)
            }
            _ => self.styles.format_placeholder(state, cursor),
        }
    }

    fn input_style(&self, state: &::cliclack::ThemeState) -> console::Style {
        self.styles.input_style(state)
    }

    fn placeholder_style(&self, state: &::cliclack::ThemeState) -> console::Style {
        self.styles.placeholder_style(state)
    }
}

/// The command's styles for cliclack's input and placeholder text.
struct Styles {
    input: console::Style,
    placeholder: console::Style,
}

impl ::cliclack::Theme for Styles {
    fn input_style(&self, _state: &::cliclack::ThemeState) -> console::Style {
        self.input.clone()
    }

    fn placeholder_style(&self, _state: &::cliclack::ThemeState) -> console::Style {
        self.placeholder.clone()
    }
}
