//! clap's styles as `console` styles, for the backends drawn with `console`.

use clap::builder::styling::{AnsiColor, Color, Effects};

use super::backend::{ClapColor, ClapStyle, NamedColor};

// A `Style`, not a `console::Color`: console carries brightness on the style.
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
