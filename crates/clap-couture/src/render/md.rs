use std::fmt::Write as _;

use clap::builder::{Styles, styling::Style};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use super::TextRenderer;

pub(crate) struct MdTextRenderer<'style> {
    styles: &'style Styles,
}

impl<'style> MdTextRenderer<'style> {
    pub(crate) const fn new(styles: &'style Styles) -> Self {
        Self { styles }
    }
}

impl TextRenderer for MdTextRenderer<'_> {
    fn render(&self, writer: &mut String, text: &str) {
        let mut strong = 0usize;
        let mut emphasis = 0usize;
        let push = |writer: &mut String,
                    strong: usize,
                    emphasis: usize,
                    base: Option<&Style>,
                    text: &str| {
            let mut style = base.copied().unwrap_or_default();
            if strong > 0 {
                style = style.bold();
            }
            if emphasis > 0 {
                style = style.italic();
            }
            if style == Style::new() {
                writer.push_str(text);
            } else {
                let _ = write!(writer, "{style}{text}{style:#}");
            }
        };
        for event in Parser::new(text) {
            match event {
                Event::Start(Tag::Strong) => strong += 1,
                Event::End(TagEnd::Strong) => strong = strong.saturating_sub(1),
                Event::Start(Tag::Emphasis) => emphasis += 1,
                Event::End(TagEnd::Emphasis) => emphasis = emphasis.saturating_sub(1),
                Event::Text(text) => push(writer, strong, emphasis, None, &text),
                Event::Code(code) => {
                    push(writer, strong, emphasis, Some(self.styles.get_literal()), &code);
                }
                Event::SoftBreak | Event::HardBreak => writer.push(' '),
                _ => {}
            }
        }
    }
}
