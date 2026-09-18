use clap::builder::Styles;

use super::TextRenderer;

pub(crate) struct PlainTextRenderer;

impl PlainTextRenderer {
    pub(crate) fn new(_styles: &Styles) -> Self {
        Self
    }
}

impl TextRenderer for PlainTextRenderer {
    fn render(&self, writer: &mut String, text: &str) {
        writer.push_str(text);
    }
}
