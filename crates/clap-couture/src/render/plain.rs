use core::fmt;

use clap::builder::{StyledStr, Styles};

use super::TextRenderer;

pub(crate) struct PlainTextRenderer;

impl PlainTextRenderer {
    pub(crate) const fn new(_styles: &Styles) -> Self {
        Self
    }
}

impl TextRenderer for PlainTextRenderer {
    fn render_str<W>(&self, out: &mut W, text: &str) -> fmt::Result
    where
        W: fmt::Write,
    {
        out.write_str(text)
    }

    fn restyle(&self, text: &StyledStr) -> StyledStr {
        text.clone()
    }

    fn restyle_line(&self, text: &StyledStr) -> StyledStr {
        text.clone()
    }

    fn restyle_wrapped(&self, wrapped: &str) -> String {
        wrapped.to_owned()
    }
}
