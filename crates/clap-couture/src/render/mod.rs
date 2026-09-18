#[cfg(feature = "markdown")]
mod md;
#[cfg(not(feature = "markdown"))]
mod plain;

use std::fmt::Write as _;

use clap::builder::Styles;
#[cfg(feature = "markdown")]
use md::MdTextRenderer as Renderer;
#[cfg(not(feature = "markdown"))]
use plain::PlainTextRenderer as Renderer;
use unicode_width::UnicodeWidthStr as _;

use crate::CommandCategoryMap;

pub(crate) trait TextRenderer {
    fn render(&self, writer: &mut String, text: &str);
}

pub(crate) fn render_groups(
    out: &mut String,
    commands: &[(String, Option<String>)],
    categories: CommandCategoryMap,
    styles: &Styles,
) {
    let renderer = Renderer::new(styles);
    let longest = commands.iter().map(|entry| entry.0.width()).max().unwrap_or(0);

    let shown = |label: &str| -> bool {
        commands.iter().any(|entry| categories.find(&entry.0).is_some_and(|c| c.label == label))
    };

    let render_rows =
        |out: &mut String, members: &mut dyn Iterator<Item = &(String, Option<String>)>| {
            let literal = styles.get_literal();
            for (index, entry) in members.enumerate() {
                if index > 0 {
                    out.push('\n');
                }
                out.push_str("  ");
                let name = entry.0.as_str();
                let pad = longest.saturating_sub(name.width());
                write!(out, "{literal}{name}{literal:#}{:pad$}", "").ok();
                if let Some(about) = entry.1.as_deref() {
                    out.push_str("  ");
                    renderer.render(out, about);
                }
            }
        };

    let heading_width = categories
        .iter()
        .filter(|entry| shown(entry.1.label))
        .map(|entry| entry.1.title.unwrap_or(entry.1.label).width().saturating_add(1))
        .max()
        .unwrap_or(0);

    let command_description_col = longest.saturating_add(4);
    let mut description_col = heading_width.saturating_add(2);
    if description_col.abs_diff(command_description_col) < 2 {
        description_col = command_description_col.saturating_add(2);
    }

    // Render uncategorized commands first, if any; a category block is prefixed
    // with a blank-line separator only once something precedes it.
    let has_uncategorized = commands.iter().any(|entry| categories.find(&entry.0).is_none());
    if has_uncategorized {
        render_rows(out, &mut commands.iter().filter(|entry| categories.find(&entry.0).is_none()));
    }
    let mut first = !has_uncategorized;

    for (i, entry) in categories.iter().enumerate() {
        let label = entry.1.label;
        let first_occurrence = !categories.iter().take(i).any(|prev| prev.1.label == label);
        if !first_occurrence || !shown(label) {
            continue;
        }
        if !first {
            out.push_str("\n\n");
        }
        first = false;

        let heading = entry.1.title.unwrap_or(label);
        let header = styles.get_header();
        write!(out, "{header}{heading}:{header:#}").ok();
        if let Some(description) = entry.1.description {
            let pad = description_col.saturating_sub(heading.width().saturating_add(1));
            write!(out, "{:pad$}", "").ok();
            renderer.render(out, description);
        }
        out.push('\n');
        render_rows(
            out,
            &mut commands
                .iter()
                .filter(|cmd| categories.find(&cmd.0).is_some_and(|c| c.label == label)),
        );
    }
}
