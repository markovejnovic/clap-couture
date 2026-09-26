//! The terminal UIs that ask for marked args.

#[cfg(feature = "interactive-cliclack")]
mod cliclack;
#[cfg(feature = "interactive-dialoguer")]
mod dialoguer;
#[cfg(feature = "interactive-inquire")]
mod inquire;

use std::io::{self, IsTerminal as _, Write as _};

use clap::builder::Styles;
#[cfg(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))]
use clap::builder::styling::{AnsiColor, Color, Effects};
#[cfg(feature = "interactive-cliclack")]
pub use cliclack::Cliclack;
#[cfg(feature = "interactive-dialoguer")]
pub use dialoguer::Dialoguer;
#[cfg(feature = "interactive-inquire")]
pub use inquire::Inquire;

use super::{ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

/// The item a backend adds to an optional select; picking it leaves the arg unset.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
const NONE_OPTION: &str = "(none)";

/// A command's clap styles, for a backend to build its theme from.
#[derive(Clone, Copy, Debug)]
pub struct CommandStyles<'styles>(pub &'styles Styles);

/// A clap ANSI color, for a backend to convert into its own color type.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub(crate) struct NamedColor(pub(crate) clap::builder::styling::AnsiColor);

/// A clap color, for a backend to convert into its own color type.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub(crate) struct ClapColor(pub(crate) clap::builder::styling::Color);

/// A clap style, for a backend to convert into its own style type.
#[cfg(any(
    feature = "interactive-cliclack",
    feature = "interactive-dialoguer",
    feature = "interactive-inquire"
))]
pub(crate) struct ClapStyle<'style>(pub(crate) &'style clap::builder::styling::Style);

// A `Style`, not a `console::Color`: console carries brightness on the style.
#[cfg(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))]
impl From<NamedColor> for console::Style {
    fn from(NamedColor(ansi): NamedColor) -> Self {
        let base = match ansi {
            AnsiColor::Black | AnsiColor::BrightBlack => console::Color::Black,
            AnsiColor::Red | AnsiColor::BrightRed => console::Color::Red,
            AnsiColor::Green | AnsiColor::BrightGreen => console::Color::Green,
            AnsiColor::Yellow | AnsiColor::BrightYellow => console::Color::Yellow,
            AnsiColor::Blue | AnsiColor::BrightBlue => console::Color::Blue,
            AnsiColor::Magenta | AnsiColor::BrightMagenta => console::Color::Magenta,
            AnsiColor::Cyan | AnsiColor::BrightCyan => console::Color::Cyan,
            AnsiColor::White | AnsiColor::BrightWhite => console::Color::White,
        };
        let style = Self::new().fg(base);
        if ansi.is_bright() { style.bright() } else { style }
    }
}

#[cfg(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))]
impl From<ClapColor> for console::Style {
    fn from(ClapColor(color): ClapColor) -> Self {
        match color {
            Color::Ansi(ansi) => Self::from(NamedColor(ansi)),
            Color::Ansi256(index) => Self::new().color256(index.0),
            Color::Rgb(rgb) => Self::new().true_color(rgb.0, rgb.1, rgb.2),
        }
    }
}

// Carries only the foreground color and the bold, dim, italic and underline effects.
#[cfg(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))]
impl From<ClapStyle<'_>> for console::Style {
    fn from(ClapStyle(style): ClapStyle<'_>) -> Self {
        let effects: [(Effects, fn(Self) -> Self); 4] = [
            (Effects::BOLD, Self::bold),
            (Effects::DIMMED, Self::dim),
            (Effects::ITALIC, Self::italic),
            (Effects::UNDERLINE, Self::underlined),
        ];
        let base =
            style.get_fg_color().map_or_else(Self::new, |color| Self::from(ClapColor(color)));
        effects
            .into_iter()
            .filter(|(effect, _)| style.get_effects().contains(*effect))
            .fold(base, |style, (_, apply)| apply(style))
    }
}

/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(feature = "interactive-cliclack")]
pub type DefaultBackend = Cliclack;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(all(feature = "interactive-dialoguer", not(feature = "interactive-cliclack")))]
pub type DefaultBackend = Dialoguer;
/// The backend [`CoutureParser`](crate::CoutureParser)'s entry points ask through: the first
/// enabled of cliclack, dialoguer and inquire.
#[cfg(all(
    feature = "interactive-inquire",
    not(any(feature = "interactive-cliclack", feature = "interactive-dialoguer"))
))]
pub type DefaultBackend = Inquire;

/// A terminal UI that can ask for one arg's value.
///
/// Implement it to bring your own UI, or to answer from a script in tests.
pub trait Backend {
    /// Ask a yes/no question.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError>;

    /// Check whether there is a terminal to ask on: stdin and stderr must both be one.
    fn is_available(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    /// Show clap's complaint about the previous answer, before asking again.
    ///
    /// # Errors
    /// - [`PromptError::Io`] when the message cannot be shown.
    fn report(&self, error: &str, _styles: CommandStyles<'_>) -> Result<(), PromptError> {
        writeln!(io::stderr(), "{error}").map_err(PromptError::Io)
    }

    /// Ask to pick one of `prompt.options`, returning the picked option's name, or `None` when an
    /// optional prompt is answered with none of them.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError>;

    /// Ask for free text, returning `None` only for an empty answer to an optional prompt.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the user backs out.
    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError>;
}
