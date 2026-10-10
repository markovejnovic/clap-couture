#[cfg(feature = "markdown")]
mod md;
#[cfg(not(feature = "markdown"))]
mod plain;

use core::{
    fmt::{self, Display},
    iter,
};

use clap::{Command, builder::Styles};
#[cfg(feature = "markdown")]
use md::MdTextRenderer as Renderer;
#[cfg(not(feature = "markdown"))]
use plain::PlainTextRenderer as Renderer;
use unicode_width::UnicodeWidthStr;

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
}

pub(crate) trait CommandRenderExt {
    /// Writes this command's visible subcommands to `out`.
    fn write_command_sections<W>(&self, out: &mut W, categories: CommandCategoryMap) -> fmt::Result
    where
        W: fmt::Write;
}

impl CommandRenderExt for Command {
    fn write_command_sections<W>(&self, out: &mut W, categories: CommandCategoryMap) -> fmt::Result
    where
        W: fmt::Write,
    {
        let menu = MenuView::new(self, categories, Renderer::new(self.get_styles()));
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

    /// The visible subcommands filed under `category` (`None` for the uncategorized ones).
    fn members(&self, category: Option<&Category>) -> impl Iterator<Item = &'cmd Command> {
        let label = category.map(|cat| cat.label);
        self.subcommands()
            .filter(move |cmd| self.category_map.find(cmd.get_name()).map(|cat| cat.label) == label)
    }

    /// The parent's subcommands, minus the hidden ones.
    fn subcommands(&self) -> impl Iterator<Item = &'cmd Command> {
        self.parent.get_subcommands().filter(|cmd| !cmd.is_hide_set())
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
        write!(f, "{header}{heading}:{header:#}")?;
        self.category.description.map_or(Ok(()), |description| {
            let col = self.view.layout.description_col;
            let pad = col.saturating_sub(heading.width().saturating_add(1));
            write!(f, "{:pad$}", "")?;
            self.view.renderer.render_str(f, description)
        })
    }
}

/// Column positions shared by every section of a menu.
struct MenuLayout {
    /// Where a category description starts.
    description_col: usize,
    /// The widest command name; abouts are aligned two cells past it.
    name_width: usize,
}

impl MenuLayout {
    fn measure<'name, 'heading>(
        names: impl Iterator<Item = &'name str>,
        headings: impl Iterator<Item = &'heading str>,
    ) -> Self {
        let name_width = names.map(UnicodeWidthStr::width).max().unwrap_or(0);
        let heading_width = headings.map(|h| h.width().saturating_add(1)).max().unwrap_or(0);

        // A description at or next to the about column reads as another row, so push it two
        // cells past.
        let about_col = name_width.saturating_add(4);
        let description_col = heading_width.saturating_add(2);
        let description_col = if description_col.abs_diff(about_col) < 2 {
            about_col.saturating_add(2)
        } else {
            description_col
        };

        Self { description_col, name_width }
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
/// ```
struct MenuView<'cmd, R> {
    layout: MenuLayout,
    menu: Menu<'cmd>,
    renderer: R,
    styles: &'cmd Styles,
}

impl<'cmd, R> MenuView<'cmd, R> {
    fn new(parent: &'cmd Command, categories: CommandCategoryMap, renderer: R) -> Self {
        let menu = Menu { category_map: categories, parent };
        let layout = MenuLayout::measure(
            menu.subcommands().map(Command::get_name),
            menu.categories().map(Category::heading),
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
        write_joined(f, uncategorized.into_iter().chain(categorized), "\n\n")
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
            self.view.renderer.render_display(f, about)
        })
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
