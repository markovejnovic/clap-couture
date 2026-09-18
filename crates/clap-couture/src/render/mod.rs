use std::fmt::Write as _;

use clap::builder::Styles;
use unicode_width::UnicodeWidthStr as _;

use crate::CommandCategoryMap;

#[cfg(feature = "markdown")]
mod md;
#[cfg(not(feature = "markdown"))]
mod plain;

#[cfg(feature = "markdown")]
use md::MdTextRenderer as Renderer;
#[cfg(not(feature = "markdown"))]
use plain::PlainTextRenderer as Renderer;

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
    let longest = commands.iter().map(|(name, _)| name.width()).max().unwrap_or(0);

    let shown = |label: &str| -> bool {
        commands.iter().any(|(name, _)| categories.find(name).is_some_and(|c| c.label == label))
    };

    let render_rows =
        |out: &mut String, members: &mut dyn Iterator<Item = &(String, Option<String>)>| {
            let literal = styles.get_literal();
            for (index, (name, about)) in members.enumerate() {
                if index > 0 {
                    out.push('\n');
                }
                out.push_str("  ");
                let pad = longest.saturating_sub(name.width());
                let _ = write!(out, "{literal}{name}{literal:#}{:pad$}", "");
                if let Some(about) = about {
                    out.push_str("  ");
                    renderer.render(out, about);
                }
            }
        };

    let heading_width = categories
        .iter()
        .filter(|(_, cat)| shown(cat.label))
        .map(|(_, cat)| cat.title.unwrap_or(cat.label).width() + 1)
        .max()
        .unwrap_or(0);

    let command_description_col = 2 + longest + 2;
    let mut description_col = heading_width + 2;
    if description_col.abs_diff(command_description_col) < 2 {
        description_col = command_description_col + 2;
    }

    let mut first = true;

    if commands.iter().any(|(name, _)| categories.find(name).is_none()) {
        render_rows(out, &mut commands.iter().filter(|(name, _)| categories.find(name).is_none()));
        first = false;
    }

    for (i, (_, cat)) in categories.iter().enumerate() {
        let label = cat.label;
        let first_occurrence = !categories.iter().take(i).any(|(_, c)| c.label == label);
        if !first_occurrence || !shown(label) {
            continue;
        }
        if !first {
            out.push_str("\n\n");
        }
        first = false;

        let heading = cat.title.unwrap_or(label);
        let header = styles.get_header();
        let _ = write!(out, "{header}{heading}:{header:#}");
        if let Some(description) = cat.description {
            let pad = description_col.saturating_sub(heading.width() + 1);
            let _ = write!(out, "{:pad$}", "");
            renderer.render(out, description);
        }
        out.push('\n');
        render_rows(
            out,
            &mut commands
                .iter()
                .filter(|(name, _)| categories.find(name).is_some_and(|c| c.label == label)),
        );
    }
}
