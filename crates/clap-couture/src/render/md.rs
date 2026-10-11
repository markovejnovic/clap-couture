use core::{
    fmt::{self, Display},
    ops::Range,
};

use clap::builder::{Styles, styling::Style};
use pulldown_cmark::{CowStr, Event, LinkType, Parser, Tag, TagEnd};

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
        let mut line = LineWriter::new(out, text, self.styles.get_literal());
        Parser::new(text).into_offset_iter().try_for_each(|(event, span)| line.write(event, span))
    }
}

/// Writes markdown events as one line of styled text, keeping every character a reader needs.
struct LineWriter<'out, 'src, W> {
    /// Whether a block or line break awaits a space before the next text.
    break_pending: bool,
    /// Whether text is a code block's.
    code_block: bool,
    /// Each open emphasis's closing delimiter when it's written as is, `None` when it styles.
    delimiters: Vec<Option<&'src str>>,
    /// Each open link's or image's destination, `None` when its text already shows it.
    destinations: Vec<Option<CowStr<'src>>>,
    emphasis: usize,
    literal: Style,
    out: &'out mut W,
    source: &'src str,
    strong: usize,
    written: bool,
}

impl<'out, 'src, W> LineWriter<'out, 'src, W>
where
    W: fmt::Write,
{
    fn close(&mut self, tag: TagEnd) -> fmt::Result {
        match tag {
            TagEnd::Emphasis => self.close_emphasis(|line| &mut line.emphasis),
            TagEnd::Strong => self.close_emphasis(|line| &mut line.strong),
            TagEnd::Link | TagEnd::Image => {
                let destination = self.destinations.pop().flatten();
                destination.map_or(Ok(()), |url| self.write_lines(&format!(" ({url})"), None))
            }
            TagEnd::CodeBlock => {
                self.code_block = false;
                self.break_pending = true;
                Ok(())
            }
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::MetadataBlock(_) => {
                self.break_pending = true;
                Ok(())
            }
            TagEnd::Strikethrough | TagEnd::Superscript | TagEnd::Subscript => Ok(()),
        }
    }

    /// Closes the innermost emphasis, whose depth counter `depth` picks.
    fn close_emphasis(&mut self, depth: fn(&mut Self) -> &mut usize) -> fmt::Result {
        if let Some(closing) = self.delimiters.pop().flatten() {
            self.write_lines(closing, None)
        } else {
            let depth = depth(self);
            *depth = depth.saturating_sub(1);
            Ok(())
        }
    }

    const fn new(out: &'out mut W, source: &'src str, literal: &Style) -> Self {
        Self {
            break_pending: false,
            code_block: false,
            delimiters: Vec::new(),
            destinations: Vec::new(),
            emphasis: 0,
            literal: *literal,
            out,
            source,
            strong: 0,
            written: false,
        }
    }

    fn open(&mut self, tag: Tag<'src>, span: Range<usize>) -> fmt::Result {
        match tag {
            Tag::Emphasis => self.open_emphasis(span, 1),
            Tag::Strong => self.open_emphasis(span, 2),
            Tag::Link { link_type, dest_url, title: _, id: _ } => {
                let shown = matches!(link_type, LinkType::Autolink | LinkType::Email);
                self.destinations.push((!shown && !dest_url.is_empty()).then_some(dest_url));
                Ok(())
            }
            Tag::Image { link_type: _, dest_url, title: _, id: _ } => {
                self.destinations.push((!dest_url.is_empty()).then_some(dest_url));
                Ok(())
            }
            // The marker (`-`, `3.`) is the item's first word in the source.
            Tag::Item => {
                self.break_pending = true;
                let marker = self.source.get(span).and_then(|item| item.split_whitespace().next());
                marker.map_or(Ok(()), |marker| self.write_lines(marker, None))?;
                self.break_pending = true;
                Ok(())
            }
            Tag::CodeBlock(_) => {
                self.code_block = true;
                self.break_pending = true;
                Ok(())
            }
            Tag::Paragraph
            | Tag::Heading { level: _, id: _, classes: _, attrs: _ }
            | Tag::BlockQuote(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::MetadataBlock(_) => {
                self.break_pending = true;
                Ok(())
            }
            Tag::Strikethrough | Tag::Superscript | Tag::Subscript => Ok(()),
        }
    }

    /// Opens an emphasis whose delimiters are `width` long; inside a word (`2*3*4`) its
    /// delimiters are text, not markup.
    fn open_emphasis(&mut self, span: Range<usize>, width: usize) -> fmt::Result {
        let before = self.source.get(..span.start).and_then(|text| text.chars().next_back());
        let after = self.source.get(span.end..).and_then(|text| text.chars().next());
        let in_word = [before, after].into_iter().flatten().any(char::is_alphanumeric);
        let opening = self.source.get(span.start..span.start.saturating_add(width));
        let closing = self.source.get(span.end.saturating_sub(width)..span.end);
        if let (true, Some(opening), Some(closing)) = (in_word, opening, closing) {
            self.delimiters.push(Some(closing));
            self.write_lines(opening, None)
        } else {
            self.delimiters.push(None);
            let depth = if width == 1 { &mut self.emphasis } else { &mut self.strong };
            *depth = depth.saturating_add(1);
            Ok(())
        }
    }

    /// Writes `event`, which `span` locates in the source.
    fn write(&mut self, event: Event<'src>, span: Range<usize>) -> fmt::Result {
        match event {
            Event::Start(tag) => self.open(tag, span),
            Event::End(tag) => self.close(tag),
            Event::Text(text) if self.code_block => self.write_lines(&text, Some(self.literal)),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.write_lines(&text, None)
            }
            Event::Code(code) => self.write_lines(&code, Some(self.literal)),
            Event::SoftBreak | Event::HardBreak => {
                self.break_pending = true;
                Ok(())
            }
            Event::Rule
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_)
            | Event::TaskListMarker(_) => self.write_source(span),
        }
    }

    /// Writes `text` over `base` (plain when `None`), with each newline as a break.
    fn write_lines(&mut self, text: &str, base: Option<Style>) -> fmt::Result {
        text.split('\n').enumerate().try_for_each(|(index, line)| {
            self.break_pending |= index > 0;
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                return Ok(());
            }
            if self.break_pending && self.written {
                self.out.write_char(' ')?;
            }
            self.break_pending = false;
            self.written = true;
            self.write_styled(line, base)
        })
    }

    /// Writes the source `span` covers as is, set apart by breaks.
    fn write_source(&mut self, span: Range<usize>) -> fmt::Result {
        let source = self.source;
        source.get(span).map_or(Ok(()), |text| {
            self.break_pending = true;
            self.write_lines(text.trim(), None)?;
            self.break_pending = true;
            Ok(())
        })
    }

    fn write_styled(&mut self, text: &str, base: Option<Style>) -> fmt::Result {
        let mut style = base.unwrap_or_default();
        if self.strong > 0 {
            style = style.bold();
        }
        if self.emphasis > 0 {
            style = style.italic();
        }
        if style == Style::new() {
            self.out.write_str(text)
        } else {
            write!(self.out, "{style}{text}{style:#}")
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn rendered(text: &str) -> String {
        let styles = Styles::plain();
        let mut out = String::new();
        let written = MdTextRenderer::new(&styles).render_str(&mut out, text);
        assert!(written.is_ok(), "writing to a `String` can't fail");
        out
    }

    #[rstest]
    #[case::plain("Ship it", "Ship it")]
    #[case::placeholder("Delete <id> from the store", "Delete <id> from the store")]
    #[case::closing_tag("Wrap it in </id>", "Wrap it in </id>")]
    #[case::html_block("<div>\nraw\n</div>", "<div> raw </div>")]
    #[case::intraword_asterisks("Multiply 2*3*4", "Multiply 2*3*4")]
    #[case::intraword_double_asterisks("Raise 2**8**", "Raise 2**8**")]
    #[case::paragraphs("First para.\n\nSecond para.", "First para. Second para.")]
    #[case::soft_break("One\ntwo", "One two")]
    #[case::hard_break("One  \ntwo", "One two")]
    #[case::link("See [the docs](https://docs.rs)", "See the docs (https://docs.rs)")]
    #[case::autolink("See <https://docs.rs>", "See https://docs.rs")]
    #[case::email("Mail <me@example.com>", "Mail me@example.com")]
    #[case::image("A ![logo](logo.png) here", "A logo (logo.png) here")]
    #[case::code("Run `make`", "Run make")]
    #[case::code_block("Run:\n\n```\nmake\nmake install\n```", "Run: make make install")]
    #[case::indented_code_block("Run:\n\n    make\n", "Run: make")]
    #[case::bullet_list("- one\n- two", "- one - two")]
    #[case::ordered_list("3. one\n4. two", "3. one 4. two")]
    #[case::nested_list("- one\n  - two", "- one - two")]
    #[case::heading("# Title\nBody", "Title Body")]
    #[case::block_quote("> Quoted", "Quoted")]
    #[case::rule("Above\n\n---\n\nBelow", "Above --- Below")]
    #[case::trailing_newline("Ship it\n", "Ship it")]
    fn renders_on_one_line_without_losing_text(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(rendered(text), expected);
    }

    #[rstest]
    #[case::strong("**bold**", "\u{1b}[1mbold\u{1b}[0m")]
    #[case::emphasis("*it*", "\u{1b}[3mit\u{1b}[0m")]
    #[case::emphasis_between_words("a *b* c", "a \u{1b}[3mb\u{1b}[0m c")]
    #[case::emphasis_in_parentheses("(*b*)", "(\u{1b}[3mb\u{1b}[0m)")]
    fn styles_emphasis(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(rendered(text), expected);
    }
}
