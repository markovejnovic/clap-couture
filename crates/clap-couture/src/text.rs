//! User-facing text the `#[couture(...)]` attributes take.

use alloc::borrow::Cow;

/// User-facing text from a `#[couture(...)]` attribute.
///
/// A string literal is kept as is. Any other expression, eg. `title = t!("deploy")`, mirrors
/// clap's `about = expr`: it is evaluated each time the text is shown, so it follows a locale
/// picked at runtime.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Text {
    /// An expression evaluated each time the text is shown.
    Computed(fn() -> Cow<'static, str>),
    /// A string literal.
    Literal(&'static str),
}

impl Text {
    /// The text to show now.
    #[must_use]
    pub fn get(&self) -> Cow<'static, str> {
        match self {
            Self::Computed(compute) => compute(),
            Self::Literal(literal) => Cow::Borrowed(literal),
        }
    }
}
