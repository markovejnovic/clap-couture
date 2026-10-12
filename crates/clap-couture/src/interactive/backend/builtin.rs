//! What the built-in backends share.

#[cfg(any(feature = "interactive-dialoguer", feature = "interactive-inquire"))]
use alloc::borrow::Cow;

#[cfg(any(feature = "interactive-dialoguer", feature = "interactive-inquire"))]
use clap::builder::styling::{Effects, Style};

#[cfg(any(feature = "interactive-dialoguer", feature = "interactive-inquire"))]
use crate::interactive::CommandStyles;
#[cfg(feature = "interactive-editor")]
use crate::interactive::PromptError;

/// How an editor prompt ended, for the line a backend leaves in the transcript.
#[cfg(feature = "interactive-editor")]
pub(super) enum Edited {
    /// Answered, shown as the answer's first line, with `…` when more follow; empty for an
    /// optional arg left unset.
    Answered(String),
    /// Backed out of, or answered with nothing where an answer is required.
    Cancelled,
    /// The editor failed; its error is reported once parsing ends.
    Failed,
}

#[cfg(feature = "interactive-editor")]
impl Edited {
    /// How `answer`, the editor's outcome for a prompt that is `optional` or not, ended.
    pub(super) fn of(answer: &Result<Option<String>, PromptError>, optional: bool) -> Self {
        match answer {
            Ok(Some(answer)) => Self::Answered(match answer.split_once('\n') {
                Some((first_line, _)) => format!("{first_line}\u{2026}"),
                None => answer.clone(),
            }),
            Ok(None) if optional => Self::Answered(String::new()),
            Ok(None) | Err(PromptError::Cancelled | PromptError::Signalled(_)) => Self::Cancelled,
            Err(_) => Self::Failed,
        }
    }
}

#[cfg(any(feature = "interactive-dialoguer", feature = "interactive-inquire"))]
impl CommandStyles<'_> {
    /// `question` as it shows in the header style of a backend that draws the header's `drawn`
    /// effects: each reset in a styled question restores that style, which would otherwise end at
    /// its first styled span.
    pub(super) fn question_in_header(self, question: &str, drawn: Effects) -> Cow<'_, str> {
        const RESET: &str = "\u{1b}[0m";
        if !question.contains(RESET) {
            return Cow::Borrowed(question);
        }
        let header = self.0.get_header();
        let effects = [Effects::BOLD, Effects::DIMMED, Effects::ITALIC, Effects::UNDERLINE]
            .into_iter()
            .filter(|effect| drawn.contains(*effect) && header.get_effects().contains(*effect))
            .fold(Effects::new(), |effects, effect| effects | effect);
        let restored = Style::new().fg_color(header.get_fg_color()).effects(effects);
        Cow::Owned(question.replace(RESET, &format!("{RESET}{restored}")))
    }
}

/// A clap ANSI color, for a backend to convert into its own color type.
pub(super) struct NamedColor(pub(super) clap::builder::styling::AnsiColor);

/// A clap color, for a backend to convert into its own color type.
pub(super) struct ClapColor(pub(super) clap::builder::styling::Color);

/// A clap style, for a backend to convert into its own style type.
pub(super) struct ClapStyle<'style>(pub(super) &'style clap::builder::styling::Style);

#[cfg(test)]
#[cfg(any(feature = "interactive-dialoguer", feature = "interactive-inquire"))]
mod tests {
    use clap::builder::{
        Styles,
        styling::{AnsiColor, Effects},
    };
    use rstest::rstest;

    use crate::interactive::CommandStyles;

    fn styles() -> Styles {
        Styles::styled()
            .header(AnsiColor::Magenta.on_default().effects(Effects::BOLD | Effects::UNDERLINE))
    }

    #[rstest]
    #[case::plain_is_kept("Your name", Effects::BOLD, "Your name")]
    #[case::a_reset_restores_the_drawn_header(
        "Your \u{1b}[1mname\u{1b}[0m, in",
        Effects::BOLD,
        "Your \u{1b}[1mname\u{1b}[0m\u{1b}[1m\u{1b}[35m, in"
    )]
    #[case::only_the_effects_the_backend_draws(
        "a\u{1b}[0mb",
        Effects::BOLD | Effects::ITALIC | Effects::UNDERLINE,
        "a\u{1b}[0m\u{1b}[1m\u{1b}[4m\u{1b}[35mb"
    )]
    fn question_in_header(#[case] question: &str, #[case] drawn: Effects, #[case] expected: &str) {
        let styles = styles();
        assert_eq!(CommandStyles(&styles).question_in_header(question, drawn), expected);
    }
}
