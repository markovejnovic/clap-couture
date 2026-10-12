use core::{fmt, ops::Range};

use clap::builder::{StyledStr, Styles, styling::Style};
use pulldown_cmark::{CowStr, Event, LinkType, Parser, Tag, TagEnd};

use super::{TextRenderer, Word};

pub(crate) struct MdTextRenderer<'style> {
    styles: &'style Styles,
}

impl<'style> MdTextRenderer<'style> {
    pub(crate) const fn new(styles: &'style Styles) -> Self {
        Self { styles }
    }
}

impl TextRenderer for MdTextRenderer<'_> {
    fn render_str<W>(&self, out: &mut W, text: &str) -> fmt::Result
    where
        W: fmt::Write,
    {
        self.render_blocks(out, text, false)
    }

    fn restyle(&self, text: &StyledStr) -> StyledStr {
        let mut rendered = StyledStr::new();
        // `StyledStr`'s `fmt::Write` is infallible.
        self.render_blocks(&mut rendered, &text.to_string(), true).ok();
        rendered
    }

    fn restyle_line(&self, text: &StyledStr) -> StyledStr {
        let mut rendered = StyledStr::new();
        // `StyledStr`'s `fmt::Write` is infallible.
        self.render_str(&mut rendered, &text.to_string()).ok();
        rendered
    }

    fn restyle_wrapped(&self, wrapped: &str) -> String {
        Word(wrapped).with_styles_reopened()
    }
}

impl MdTextRenderer<'_> {
    /// Writes `text`, keeping paragraphs and list items apart when `keep_blocks`, else on one line.
    fn render_blocks<W>(&self, out: &mut W, text: &str, keep_blocks: bool) -> fmt::Result
    where
        W: fmt::Write,
    {
        let mut line = LineWriter::new(out, text, self.styles.get_literal(), keep_blocks);
        Parser::new(text).into_offset_iter().try_for_each(|(event, span)| line.write(event, span))
    }
}

/// Writes markdown events as one line of styled text, keeping every character a reader needs.
struct LineWriter<'out, 'src, W> {
    /// What the pending break writes instead of a space, after a paragraph or list item.
    block_break: Option<&'static str>,
    /// Whether a block or line break awaits a space before the next text.
    break_pending: bool,
    /// Whether text is a code block's.
    code_block: bool,
    /// Each open link's or image's destination, `None` when its text already shows it.
    destinations: Vec<Option<CowStr<'src>>>,
    emphasis: usize,
    /// What ends a list item: a newline when blocks are kept, else a space.
    item_break: &'static str,
    literal: Style,
    out: &'out mut W,
    /// What ends a paragraph or heading: a blank line when blocks are kept, else a space.
    paragraph_break: &'static str,
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
            TagEnd::Emphasis => {
                self.emphasis = self.emphasis.saturating_sub(1);
                Ok(())
            }
            TagEnd::Strong => {
                self.strong = self.strong.saturating_sub(1);
                Ok(())
            }
            TagEnd::Link | TagEnd::Image => {
                let destination = self.destinations.pop().flatten();
                destination.map_or(Ok(()), |url| self.write_lines(&format!(" ({url})"), None))
            }
            TagEnd::CodeBlock => {
                self.code_block = false;
                self.break_pending = true;
                Ok(())
            }
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item => {
                self.break_pending = true;
                self.block_break = Some(if matches!(tag, TagEnd::Item) {
                    self.item_break
                } else {
                    self.paragraph_break
                });
                Ok(())
            }
            TagEnd::BlockQuote(_)
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
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

    const fn new(out: &'out mut W, source: &'src str, literal: &Style, keep_blocks: bool) -> Self {
        Self {
            block_break: None,
            break_pending: false,
            code_block: false,
            destinations: Vec::new(),
            emphasis: 0,
            item_break: if keep_blocks { "\n" } else { " " },
            literal: *literal,
            out,
            paragraph_break: if keep_blocks { "\n\n" } else { " " },
            source,
            strong: 0,
            written: false,
        }
    }

    fn open(&mut self, tag: Tag<'src>, span: Range<usize>) -> fmt::Result {
        match tag {
            Tag::Emphasis => {
                self.emphasis = self.emphasis.saturating_add(1);
                Ok(())
            }
            Tag::Strong => {
                self.strong = self.strong.saturating_add(1);
                Ok(())
            }
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
                self.out.write_str(self.block_break.unwrap_or(" "))?;
            }
            self.block_break = None;
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
    #[case::paragraphs("First para.\n\nSecond para.", "First para.\n\nSecond para.")]
    #[case::heading("# Title\nBody", "Title\n\nBody")]
    #[case::bullet_list("- one\n- two", "- one\n- two")]
    #[case::soft_break("One\ntwo", "One two")]
    fn restyle_keeps_paragraphs_and_items_apart(#[case] text: &str, #[case] expected: &str) {
        let styles = Styles::plain();
        let restyled = MdTextRenderer::new(&styles).restyle(&StyledStr::from(text.to_owned()));
        assert_eq!(restyled.to_string(), expected);
    }

    #[rstest]
    #[case::strong("**bold**", "\u{1b}[1mbold\u{1b}[0m")]
    #[case::emphasis("*it*", "\u{1b}[3mit\u{1b}[0m")]
    #[case::emphasis_between_words("a *b* c", "a \u{1b}[3mb\u{1b}[0m c")]
    #[case::emphasis_in_parentheses("(*b*)", "(\u{1b}[3mb\u{1b}[0m)")]
    #[case::emphasis_inside_a_word("2*3*4", "2\u{1b}[3m3\u{1b}[0m4")]
    #[case::strong_before_a_word("**bold**text", "\u{1b}[1mbold\u{1b}[0mtext")]
    fn styles_emphasis(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(rendered(text), expected);
    }
}
