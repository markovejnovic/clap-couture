mod help;
#[cfg(feature = "markdown")]
mod md;
#[cfg(not(feature = "markdown"))]
mod plain;

use alloc::borrow::Cow;
use core::{
    fmt::{self, Display},
    iter,
};

#[cfg(feature = "interactive")]
use clap::ColorChoice;
use clap::{
    Command,
    builder::{Resettable, StyledStr, Styles},
};
#[cfg(feature = "markdown")]
use md::MdTextRenderer as Renderer;
#[cfg(not(feature = "markdown"))]
use plain::PlainTextRenderer as Renderer;
use unicode_width::UnicodeWidthStr;

pub(crate) use self::help::{HelpBody, HelpTemplates};
use crate::{Category, CommandCategoryMap};

/// Styles a run of help text (a command's about or a category description).
pub(crate) trait TextRenderer {
    fn render_str<W>(&self, out: &mut W, text: &str) -> fmt::Result
    where
        W: fmt::Write;

    /// `text` as this renderer shows it, as text clap writes as is.
    fn restyle(&self, text: &StyledStr) -> StyledStr;

    /// `text` as this renderer shows it in a subcommand listing's row.
    fn restyle_line(&self, text: &StyledStr) -> StyledStr;

    /// `wrapped`, text clap wrapped and indented, as this renderer shows it.
    fn restyle_wrapped(&self, wrapped: &str) -> String;
}

pub(crate) trait CommandRenderExt {
    /// The width clap wraps this command's help to; `None` when it doesn't wrap.
    ///
    /// clap's build hands a command's `term_width` and `max_term_width` to each subcommand, so
    /// the built root's width is every built command's.
    fn help_width(&self) -> Option<usize>;

    /// `text` rendered on one line as a prompt's question: styled when this command's color is on
    /// for stderr, else plain.
    #[cfg(feature = "interactive")]
    fn render_question(&self, text: &str) -> String;

    /// This command with its about and long about rendered.
    #[must_use]
    fn with_rendered_about(self) -> Self;

    /// This command with each arg's help and long help rendered.
    #[must_use]
    fn with_rendered_arg_help(self) -> Self;

    /// Writes this command's visible subcommands to `out`, wrapped to `width`.
    fn write_command_sections<W>(
        &self,
        out: &mut W,
        categories: CommandCategoryMap,
        width: Option<usize>,
    ) -> fmt::Result
    where
        W: fmt::Write;
}

impl CommandRenderExt for Command {
    fn help_width(&self) -> Option<usize> {
        // clap has no getter for it: it follows `term_width`, `max_term_width`, the terminal and
        // `COLUMNS`, and wraps only with its `wrap_help` feature. So clap wraps a probe: a line
        // breaks after the last word ending within the width, and the words of the `a` line end
        // at odd columns while those of the `b` line end at even ones.
        const PROBE_WORDS: usize = 500;
        let odd = vec!["a"; PROBE_WORDS].join(" ");
        let even = format!("b{}", vec!["b"; PROBE_WORDS].join(" "));
        let unwrapped = odd.len();
        let help = self
            .clone()
            .help_template("{after-help}")
            .after_help(format!("{odd}\n{even}"))
            .render_help()
            .to_string();
        let first_line =
            |letter: char| help.lines().find(|line| line.starts_with(letter)).map_or(0, str::len);
        let width = first_line('a').max(first_line('b'));
        (width < unwrapped).then_some(width)
    }

    #[cfg(feature = "interactive")]
    fn render_question(&self, text: &str) -> String {
        let colored = match self.get_color() {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                anstream::AutoStream::choice(&std::io::stderr()) != anstream::ColorChoice::Never
            }
        };
        let mut question = StyledStr::new();
        // `StyledStr`'s `fmt::Write` is infallible.
        Renderer::new(self.get_styles()).render_str(&mut question, text).ok();
        if colored { question.ansi().to_string() } else { question.to_string() }
    }

    fn with_rendered_about(self) -> Self {
        let renderer = Renderer::new(self.get_styles());
        let about = self.get_about().map(|text| renderer.restyle(text));
        let long_about = self.get_long_about().map(|text| renderer.restyle(text));
        self.about(Resettable::from(about)).long_about(Resettable::from(long_about))
    }

    fn with_rendered_arg_help(self) -> Self {
        let styles = self.get_styles().clone();
        let renderer = Renderer::new(&styles);
        self.mut_args(|arg| {
            let help = arg.get_help().map(|text| renderer.restyle(text));
            let long_help = arg.get_long_help().map(|text| renderer.restyle(text));
            arg.help(Resettable::from(help)).long_help(Resettable::from(long_help))
        })
    }

    fn write_command_sections<W>(
        &self,
        out: &mut W,
        categories: CommandCategoryMap,
        width: Option<usize>,
    ) -> fmt::Result
    where
        W: fmt::Write,
    {
        let menu = MenuView::new(self, categories, Renderer::new(self.get_styles()), width);
        write!(out, "{menu}")
    }
}

/// A command's subcommands as clap lists them, partitioned by category.
struct Menu<'cmd> {
    category_map: CommandCategoryMap,
    parent: &'cmd Command,
    /// Each listed subcommand's lines, in clap's order.
    rows: Vec<Row<'cmd>>,
}

impl<'cmd> Menu<'cmd> {
    /// Ends the `{subcommands}` template, so clap's trim of the help keeps the last row's padding.
    const LISTING_END: &'static str = "\u{E000}";

    /// The distinct categories with at least one visible member, in first-appearance order.
    fn categories(&self) -> impl Iterator<Item = &'static Category> {
        self.category_map
            .distinct()
            .filter(|category| self.members(Some(category)).next().is_some())
    }

    fn has_uncategorized(&self) -> bool {
        self.members(None).next().is_some()
    }

    /// The row of the `help` subcommand clap generated for the parent; `None` when it didn't.
    fn help_row(&self) -> Option<&Row<'cmd>> {
        self.rows.iter().find(|row| self.is_help_subcommand(row.cmd))
    }

    /// Check whether `cmd` is the `help` subcommand clap generated for the parent.
    fn is_help_subcommand(&self, cmd: &Command) -> bool {
        !self.parent.is_disable_help_subcommand_set() && cmd.get_name() == "help"
    }

    /// The rows of the subcommands filed under `category` (`None` for the uncategorized ones),
    /// minus clap's `help`.
    fn members(&self, category: Option<&Category>) -> impl Iterator<Item = &Row<'cmd>> {
        let label = category.map(|cat| cat.label);
        self.rows.iter().filter(move |row| {
            !self.is_help_subcommand(row.cmd)
                && self.category_map.find(row.cmd.get_name()).map(|cat| cat.label) == label
        })
    }

    /// `parent`'s subcommands as clap lists them under `Commands:`, their abouts rendered by
    /// `renderer`: none when clap lists none, as when only `help` would show.
    fn new<R>(parent: &'cmd Command, category_map: CommandCategoryMap, renderer: &R) -> Self
    where
        R: TextRenderer,
    {
        let lists_subcommands =
            parent.get_subcommands().any(|sub| sub.get_name() != "help" && !sub.is_hide_set());
        let rows = if lists_subcommands { Row::all_of(parent, renderer) } else { Vec::new() };
        Self { category_map, parent, rows }
    }
}

/// One subcommand's lines in clap's listing: its name and flags, then its about and aliases,
/// wrapped and aligned as clap does.
struct Row<'cmd> {
    cmd: &'cmd Command,
    lines: String,
}

impl<'cmd> Row<'cmd> {
    /// clap's listing of `parent`'s visible subcommands, split into each one's row.
    fn all_of<R>(parent: &'cmd Command, renderer: &R) -> Vec<Self>
    where
        R: TextRenderer,
    {
        let listing = parent
            .clone()
            .mut_subcommands(|sub| {
                // clap lists the long about when there is no short one.
                let about = sub.get_about().or_else(|| sub.get_long_about());
                let about = about.map(|about| renderer.restyle_line(about));
                sub.about(Resettable::from(about))
            })
            .help_template(format!("{{subcommands}}{}", Menu::LISTING_END))
            .render_help()
            .ansi()
            .to_string();
        let listing = listing.rsplit_once(Menu::LISTING_END).map_or(&*listing, |(rows, _)| rows);
        let listing = renderer.restyle_wrapped(listing);
        let mut rows: Vec<Self> = Vec::new();
        for line in listing.split('\n') {
            // A row starts at clap's two-space indent; the lines it wraps onto are indented past.
            let starts_row = Word(line)
                .visible()
                .strip_prefix("  ")
                .is_some_and(|rest| rest.chars().next().is_some_and(|first| first != ' '));
            let cmd = starts_row.then(|| Self::subcommand_on(parent, line)).flatten();
            match (cmd, rows.last_mut()) {
                (Some(cmd), _) => rows.push(Self { cmd, lines: line.to_owned() }),
                (None, Some(row)) => {
                    row.lines.push('\n');
                    row.lines.push_str(line);
                }
                (None, None) => {}
            }
        }
        rows
    }

    /// The cells clap gives `cmd`'s name and flags.
    fn name_width(cmd: &Command) -> usize {
        let short =
            cmd.get_short_flag().map_or(0, |short| ", -".len().saturating_add(short.len_utf8()));
        let long = cmd.get_long_flag().map_or(0, |long| ", --".len().saturating_add(long.width()));
        cmd.get_name().width().saturating_add(short).saturating_add(long)
    }

    /// The subcommand of `parent` whose row starts with `line`: the longest name it starts with,
    /// followed by its flags, its about or nothing.
    fn subcommand_on(parent: &'cmd Command, line: &str) -> Option<&'cmd Command> {
        let visible = Word(line).visible();
        let rest = visible.strip_prefix("  ")?;
        parent
            .get_subcommands()
            .filter(|sub| {
                rest.strip_prefix(sub.get_name())
                    .is_some_and(|after| after.is_empty() || after.starts_with([',', ' ']))
            })
            .max_by_key(|sub| sub.get_name().len())
    }
}
/// A category heading, optionally followed by its description.
struct HeadingView<'view, R> {
    category: &'static Category,
    view: &'view MenuView<'view, R>,
}

impl<R> Display for HeadingView<'_, R>
where
    R: TextRenderer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = self.view.styles.get_header();
        let heading = self.category.heading();
        write!(f, "{header}{heading}{header:#}")?;
        self.category.description.as_ref().map_or(Ok(()), |description| {
            let col = self.view.layout.description_col;
            let pad = col.saturating_sub(heading.width());
            write!(f, "{:pad$}", "")?;
            let mut rendered = String::new();
            self.view.renderer.render_str(&mut rendered, &description.get())?;
            // clap counts the blank line above a heading as a cell of the heading's line.
            let width = self.view.layout.width.map(|width| width.saturating_sub(1));
            write!(f, "{}", ColumnText { col, text: &rendered, width })
        })
    }
}

/// Column positions shared by every section of a menu.
struct MenuLayout {
    /// Where a category description starts.
    description_col: usize,
    /// The width lines wrap to; `None` when they don't.
    width: Option<usize>,
}

impl MenuLayout {
    /// The layout for rows with names and flags `name_widths` cells wide, under `headings`.
    fn measure(
        name_widths: impl Iterator<Item = usize>,
        headings: impl Iterator<Item = Cow<'static, str>>,
        width: Option<usize>,
    ) -> Self {
        // clap aligns abouts two cells past the widest name, and never narrower than `-x`.
        let name_width = name_widths.max().unwrap_or(0).max(2);
        let heading_width = headings.map(|h| h.width()).max().unwrap_or(0);

        // A description at or next to the about column reads as another row, so push it two
        // cells past.
        let about_col = name_width.saturating_add(4);
        let description_col = heading_width.saturating_add(2);
        let description_col = if description_col.abs_diff(about_col) < 2 {
            about_col.saturating_add(2)
        } else {
            description_col
        };

        Self { description_col, width }
    }
}

/// A command's visible subcommands, grouped under their category headings.
///
/// ```text
///   logs    Tail live logs
///   status  Show recent activity
///
/// Deploy:     Ship code to production
///   deploy  Ship the current project
///
/// Account:
///   login   Sign in to your account
///
///   help    Print this message or the help of the given subcommand(s)
/// ```
struct MenuView<'cmd, R> {
    layout: MenuLayout,
    menu: Menu<'cmd>,
    renderer: R,
    styles: &'cmd Styles,
}

impl<'cmd, R> MenuView<'cmd, R>
where
    R: TextRenderer,
{
    fn new(
        parent: &'cmd Command,
        categories: CommandCategoryMap,
        renderer: R,
        width: Option<usize>,
    ) -> Self {
        let menu = Menu::new(parent, categories, &renderer);
        let layout = MenuLayout::measure(
            menu.rows.iter().map(|row| Row::name_width(row.cmd)),
            menu.categories().map(Category::heading),
            width,
        );
        Self { layout, menu, renderer, styles: parent.get_styles() }
    }
}

impl<R> Display for MenuView<'_, R>
where
    R: TextRenderer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let uncategorized =
            self.menu.has_uncategorized().then_some(SectionView { category: None, view: self });
        let categorized = self
            .menu
            .categories()
            .map(|category| SectionView { category: Some(category), view: self });
        let has_categories = self.menu.categories().next().is_some();
        write_joined(f, uncategorized.into_iter().chain(categorized), "\n\n")?;
        self.menu.help_row().map_or(Ok(()), |row| {
            // clap's `help` ends the uncategorized rows, or follows the categories a section apart.
            let sep = match (has_categories, self.menu.has_uncategorized()) {
                (true, _) => "\n\n",
                (false, true) => "\n",
                (false, false) => "",
            };
            write!(f, "{sep}{}", row.lines)
        })
    }
}

/// Rendered text starting at `col`, broken between words to fit `width`, each line after the
/// first indented to `col`.
struct ColumnText<'text> {
    col: usize,
    text: &'text str,
    width: Option<usize>,
}

impl Display for ColumnText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(width) = self.width else {
            return f.write_str(self.text);
        };
        let indent = self.col;
        let mut column = indent;
        // The escapes in effect, reopened past the indent of each new line.
        let mut open = String::new();
        for (index, word) in self.text.split(' ').map(Word).enumerate() {
            if index > 0 {
                if column.saturating_add(1).saturating_add(word.width()) > width {
                    if !open.is_empty() {
                        f.write_str(Word::RESET)?;
                    }
                    write!(f, "\n{:indent$}{open}", "")?;
                    column = indent;
                } else {
                    f.write_str(" ")?;
                    column = column.saturating_add(1);
                }
            }
            f.write_str(word.0)?;
            column = column.saturating_add(word.width());
            for escape in word.escapes() {
                if escape == Word::RESET {
                    open.clear();
                } else {
                    open.push_str(escape);
                }
            }
        }
        Ok(())
    }
}

/// A run of rendered text between spaces, its ANSI SGR escapes taking no cells.
struct Word<'text>(&'text str);

impl<'text> Word<'text> {
    /// The escape that ends every style.
    const RESET: &'static str = "\u{1b}[0m";

    /// The escapes in this word, in order.
    fn escapes(&self) -> impl Iterator<Item = &'text str> {
        let text = self.0;
        text.match_indices('\u{1b}').map(move |(start, _)| {
            let escape = text.get(start..).unwrap_or_default();
            escape.find('m').and_then(|end| escape.get(..=end)).unwrap_or(escape)
        })
    }

    /// The pieces of this text shown on screen, between its escapes.
    fn shown(&self) -> impl Iterator<Item = &'text str> {
        self.0.split('\u{1b}').enumerate().map(|(index, piece)| {
            if index == 0 { piece } else { piece.split_once('m').map_or("", |(_, text)| text) }
        })
    }

    /// This text without its escapes.
    fn visible(&self) -> String {
        self.shown().collect()
    }

    /// The cells this word takes on screen.
    fn width(&self) -> usize {
        self.shown().map(UnicodeWidthStr::width).sum()
    }

    /// Lines clap wrapped, each style still open at a line's end closed there and opened again
    /// past the next line's indent, so the indent is never styled.
    #[cfg(feature = "markdown")]
    fn with_styles_reopened(&self) -> String {
        let mut reopened = String::with_capacity(self.0.len());
        // The escapes in effect at the end of the line so far.
        let mut open = String::new();
        for (index, line) in self.0.split('\n').enumerate() {
            if index > 0 {
                if !open.is_empty() {
                    reopened.push_str(Self::RESET);
                }
                reopened.push('\n');
                let text = line.trim_start_matches(' ');
                reopened.push_str(line.get(..line.len().saturating_sub(text.len())).unwrap_or(""));
                reopened.push_str(&open);
                reopened.push_str(text);
            } else {
                reopened.push_str(line);
            }
            for escape in Word(line).escapes() {
                if escape == Self::RESET {
                    open.clear();
                } else {
                    open.push_str(escape);
                }
            }
        }
        reopened
    }
}

/// A category's heading and rows, or just the rows for uncategorized commands.
struct SectionView<'view, R> {
    category: Option<&'static Category>,
    view: &'view MenuView<'view, R>,
}

impl<R> Display for SectionView<'_, R>
where
    R: TextRenderer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.category.map_or(Ok(()), |category| {
            let heading = HeadingView { category, view: self.view };
            writeln!(f, "{heading}")
        })?;
        let rows = self.view.menu.members(self.category).map(|row| &row.lines);
        write_joined(f, rows, "\n")
    }
}

/// Writes each item of `items` to `out`, separated by `sep`.
fn write_joined<W, I>(out: &mut W, items: I, sep: &str) -> fmt::Result
where
    W: fmt::Write,
    I: IntoIterator,
    I::Item: Display,
{
    let separators = iter::once("").chain(iter::repeat(sep));
    items.into_iter().zip(separators).try_for_each(|(item, sep)| write!(out, "{sep}{item}"))
}
