use core::fmt::{self, Display};

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
    fn render_display<W, D>(&self, out: &mut W, text: D) -> fmt::Result
    where
        W: fmt::Write,
        D: Display,
    {
        self.render_str(out, &text.to_string())
    }

    fn render_str<W>(&self, out: &mut W, text: &str) -> fmt::Result
    where
        W: fmt::Write,
    {
        let mut strong = 0usize;
        let mut emphasis = 0usize;
        let push = |out: &mut W,
                    strong: usize,
                    emphasis: usize,
                    base: Option<&Style>,
                    text: &str|
         -> fmt::Result {
            let mut style = base.copied().unwrap_or_default();
            if strong > 0 {
                style = style.bold();
            }
            if emphasis > 0 {
                style = style.italic();
            }
            if style == Style::new() {
                out.write_str(text)
            } else {
                write!(out, "{style}{text}{style:#}")
            }
        };

        for event in Parser::new(text) {
            match event {
                Event::Start(Tag::Strong) => strong = strong.saturating_add(1),
                Event::End(TagEnd::Strong) => strong = strong.saturating_sub(1),
                Event::Start(Tag::Emphasis) => emphasis = emphasis.saturating_add(1),
                Event::End(TagEnd::Emphasis) => emphasis = emphasis.saturating_sub(1),
                Event::Text(text) => push(out, strong, emphasis, None, &text)?,
                Event::Code(code) => {
                    push(out, strong, emphasis, Some(self.styles.get_literal()), &code)?;
                }
                Event::SoftBreak | Event::HardBreak => out.write_char(' ')?,
                // Other markup is dropped; the text inside it still arrives as `Event::Text`.
                Event::Start(_)
                | Event::End(_)
                | Event::InlineMath(_)
                | Event::DisplayMath(_)
                | Event::Html(_)
                | Event::InlineHtml(_)
                | Event::FootnoteReference(_)
                | Event::Rule
                | Event::TaskListMarker(_) => {}
            }
        }
        Ok(())
    }
}
