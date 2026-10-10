use core::fmt::{self, Display};

use clap::builder::Styles;

use super::TextRenderer;

pub(crate) struct PlainTextRenderer;

impl PlainTextRenderer {
    pub(crate) const fn new(_styles: &Styles) -> Self {
        Self
    }
}

impl TextRenderer for PlainTextRenderer {
    fn render_display<W, D>(&self, out: &mut W, text: D) -> fmt::Result
    where
        W: fmt::Write,
        D: Display,
    {
        write!(out, "{text}")
    }

    fn render_str<W>(&self, out: &mut W, text: &str) -> fmt::Result
    where
        W: fmt::Write,
    {
        out.write_str(text)
    }
}
