//! clap's styles as `console` styles, for the backends drawn with `console`.

use clap::builder::styling::{AnsiColor, Color, Effects, Style};

/// A `console::Style` builder method, such as `console::Style::bold`.
type Effect = fn(console::Style) -> console::Style;

/// `style`'s foreground color and its bold, dim, italic and underline effects.
pub(crate) fn console_style(style: &Style) -> console::Style {
    let effects: [(Effects, Effect); 4] = [
        (Effects::BOLD, console::Style::bold),
        (Effects::DIMMED, console::Style::dim),
        (Effects::ITALIC, console::Style::italic),
        (Effects::UNDERLINE, console::Style::underlined),
    ];
    let base = style.get_fg_color().map_or_else(console::Style::new, color);
    effects
        .into_iter()
        .filter(|(effect, _)| style.get_effects().contains(*effect))
        .fold(base, |style, (_, apply)| apply(style))
}

fn color(color: Color) -> console::Style {
    match color {
        Color::Ansi(ansi) => ansi_color(ansi),
        Color::Ansi256(index) => console::Style::new().color256(index.0),
        Color::Rgb(rgb) => console::Style::new().true_color(rgb.0, rgb.1, rgb.2),
    }
}

const fn ansi_base(ansi: AnsiColor) -> console::Color {
    match ansi {
        AnsiColor::Black | AnsiColor::BrightBlack => console::Color::Black,
        AnsiColor::Red | AnsiColor::BrightRed => console::Color::Red,
        AnsiColor::Green | AnsiColor::BrightGreen => console::Color::Green,
        AnsiColor::Yellow | AnsiColor::BrightYellow => console::Color::Yellow,
        AnsiColor::Blue | AnsiColor::BrightBlue => console::Color::Blue,
        AnsiColor::Magenta | AnsiColor::BrightMagenta => console::Color::Magenta,
        AnsiColor::Cyan | AnsiColor::BrightCyan => console::Color::Cyan,
        AnsiColor::White | AnsiColor::BrightWhite => console::Color::White,
    }
}

fn ansi_color(ansi: AnsiColor) -> console::Style {
    let style = console::Style::new().fg(ansi_base(ansi));
    if ansi.is_bright() { style.bright() } else { style }
}
