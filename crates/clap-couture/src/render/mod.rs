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

use clap::{
    Command,
    builder::{Resettable, StyledStr, Styles},
};
#[cfg(feature = "markdown")]
use md::MdTextRenderer as Renderer;
#[cfg(not(feature = "markdown"))]
use plain::PlainTextRenderer as Renderer;
use unicode_width::UnicodeWidthStr;

pub(crate) use self::help::HelpBody;
use crate::{Category, CommandCategoryMap};

/// Styles a run of help text (a command's about or a category description).
pub(crate) trait TextRenderer {
    fn render_display<W, D>(&self, out: &mut W, text: D) -> fmt::Result
    where
        W: fmt::Write,
        D: Display;

    fn render_str<W>(&self, out: &mut W, text: &str) -> fmt::Result
    where
        W: fmt::Write;

    /// `text` as this renderer shows it, as text clap writes as is.
    fn restyle(&self, text: &StyledStr) -> StyledStr;
}

pub(crate) trait CommandRenderExt {
    /// The width clap wraps this command's help to; `None` when it doesn't wrap.
    ///
    /// clap's build hands a command's `term_width` and `max_term_width` to each subcommand, so
    /// the built root's width is every built command's.
    fn help_width(&self) -> Option<usize>;

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

/// A command's visible subcommands, partitioned by category.
struct Menu<'cmd> {
    category_map: CommandCategoryMap,
    parent: &'cmd Command,
}

impl<'cmd> Menu<'cmd> {
    /// The distinct categories with at least one visible member, in first-appearance order.
    fn categories(&self) -> impl Iterator<Item = &'static Category> {
        self.category_map
            .distinct()
            .filter(|category| self.members(Some(category)).next().is_some())
    }

    fn has_uncategorized(&self) -> bool {
        self.members(None).next().is_some()
    }

    /// The `help` subcommand clap generated for the parent; `None` when it didn't.
    fn help_subcommand(&self) -> Option<&'cmd Command> {
        self.subcommands().find(|cmd| self.is_help_subcommand(cmd))
    }

    /// Check whether `cmd` is the `help` subcommand clap generated for the parent.
    fn is_help_subcommand(&self, cmd: &Command) -> bool {
        !self.parent.is_disable_help_subcommand_set() && cmd.get_name() == "help"
    }

    /// The visible subcommands filed under `category` (`None` for the uncategorized ones), minus
    /// clap's `help`.
    fn members(&self, category: Option<&Category>) -> impl Iterator<Item = &'cmd Command> {
        let label = category.map(|cat| cat.label);
        self.subcommands().filter(move |cmd| {
            !self.is_help_subcommand(cmd)
                && self.category_map.find(cmd.get_name()).map(|cat| cat.label) == label
        })
    }

    /// The parent's subcommands, minus the hidden ones, in clap's display order.
    fn subcommands(&self) -> impl Iterator<Item = &'cmd Command> {
        let mut visible: Vec<_> =
            self.parent.get_subcommands().filter(|cmd| !cmd.is_hide_set()).collect();
        // `mut_subcommand` moves the subcommand it edits to the end, as guarding a secret does.
        visible.sort_by_key(|cmd| cmd.get_display_order());
        visible.into_iter()
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
    /// The widest command name; abouts are aligned two cells past it.
    name_width: usize,
    /// The width lines wrap to; `None` when they don't.
    width: Option<usize>,
}

impl MenuLayout {
    /// Where a command's about starts.
    const fn about_col(&self) -> usize {
        self.name_width.saturating_add(4)
    }

    fn measure<'name>(
        names: impl Iterator<Item = &'name str>,
        headings: impl Iterator<Item = Cow<'static, str>>,
        width: Option<usize>,
    ) -> Self {
        let name_width = names.map(UnicodeWidthStr::width).max().unwrap_or(0);
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

        Self { description_col, name_width, width }
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

impl<'cmd, R> MenuView<'cmd, R> {
    fn new(
        parent: &'cmd Command,
        categories: CommandCategoryMap,
        renderer: R,
        width: Option<usize>,
    ) -> Self {
        let menu = Menu { category_map: categories, parent };
        let layout = MenuLayout::measure(
            menu.subcommands().map(Command::get_name),
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
        self.menu.help_subcommand().map_or(Ok(()), |cmd| {
            // clap's `help` ends the uncategorized rows, or follows the categories a section apart.
            let sep = match (has_categories, self.menu.has_uncategorized()) {
                (true, _) => "\n\n",
                (false, true) => "\n",
                (false, false) => "",
            };
            write!(f, "{sep}{}", RowView { cmd, view: self })
        })
    }
}

/// One subcommand, with its about aligned past the widest name in the menu.
struct RowView<'view, R> {
    cmd: &'view Command,
    view: &'view MenuView<'view, R>,
}

impl<R> Display for RowView<'_, R>
where
    R: TextRenderer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let literal = self.view.styles.get_literal();
        let name = self.cmd.get_name();
        let pad = self.view.layout.name_width.saturating_sub(name.width());
        write!(f, "  {literal}{name}{literal:#}{:pad$}", "")?;
        self.cmd.get_about().map_or(Ok(()), |about| {
            f.write_str("  ")?;
            let mut rendered = String::new();
            self.view.renderer.render_display(&mut rendered, about)?;
            let (col, width) = (self.view.layout.about_col(), self.view.layout.width);
            write!(f, "{}", ColumnText { col, text: &rendered, width })
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

    /// The cells this word takes on screen.
    fn width(&self) -> usize {
        let visible = |(index, piece): (usize, &'text str)| {
            if index == 0 { piece } else { piece.split_once('m').map_or("", |(_, text)| text) }
        };
        self.0.split('\u{1b}').enumerate().map(visible).map(UnicodeWidthStr::width).sum()
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
        let rows =
            self.view.menu.members(self.category).map(|cmd| RowView { cmd, view: self.view });
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
