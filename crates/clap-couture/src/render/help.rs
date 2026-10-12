//! The `--help` couture installs: clap's own text, with the subcommands listed by category.

use clap::{Arg, Command, builder::StyledStr};

use super::CommandRenderExt as _;
use crate::{CommandCategoryMap, Messages};

/// What couture shows below the usage, at `{after-help}`: the categorized subcommands, clap's
/// arg sections, then the command's own after-help.
///
/// clap can't regroup its subcommand listing, so its `{all-args}` is rendered here from a copy of
/// the command with the subcommands hidden, once for `-h` and once for `--help`. Rendering it
/// through clap keeps positionals, `help_heading`s and hidden args exactly as clap shows them.
pub(crate) struct HelpBody {
    /// For `--help`, when clap shows more for it than for `-h`.
    long: Option<String>,
    short: String,
}

/// Rewords clap's English `Arguments:` and `Options:` headings with a command's [`Messages`].
struct Headings<'cmd> {
    cmd: &'cmd Command,
    messages: &'cmd Messages,
}

/// The help templates couture tells apart from one the app set.
pub(crate) trait HelpTemplates {
    /// clap's templates for a command without one of its own: `DEFAULT_TEMPLATE`, and
    /// `DEFAULT_NO_ARGS_TEMPLATE` for one with nothing to list (`clap_builder` 4.6.7,
    /// `src/output/help_template.rs`).
    const CLAP_DEFAULTS: [&str; 2] = [
        "{before-help}{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}{after-help}",
        "{before-help}{about-with-newline}\n{usage-heading} {usage}{after-help}",
    ];

    /// couture's template: clap's, with the usage heading from `messages`, and everything below
    /// the usage at `{after-help}`.
    fn couture_template(&self, messages: &Messages) -> String;

    /// Check whether `--help` shows more than `-h`, as clap decides it
    /// (`Command::long_help_exists_` in `clap_builder` 4.6.7): otherwise clap shows `-h`'s help
    /// for both.
    fn has_long_help(&self) -> bool;

    /// Check whether the app set its own help template. clap has no getter for it, so the help
    /// is rendered under clap's default templates and compared.
    fn has_own_template(&self) -> bool;
}

impl HelpBody {
    /// `cmd` showing this body at `{after-help}`, under couture's template unless the app set its
    /// own.
    pub(crate) fn install(self, cmd: Command, messages: &Messages) -> Command {
        let cmd = if cmd.has_own_template() {
            cmd
        } else {
            let template = cmd.couture_template(messages);
            cmd.help_template(template)
        };
        let cmd = cmd.with_rendered_about();
        // An empty after-help would still print its blank line.
        let cmd = if self.short.is_empty() { cmd } else { cmd.after_help(self.short) };
        match self.long {
            Some(long) => cmd.after_long_help(long),
            None => cmd,
        }
    }

    /// The body for `cmd`, its subcommands filed by `categories` and wrapped to `width`, its
    /// headings worded by `messages`.
    pub(crate) fn new(
        cmd: &Command,
        categories: CommandCategoryMap,
        messages: &Messages,
        width: Option<usize>,
    ) -> Self {
        let mut menu = StyledStr::new();
        // `StyledStr`'s `fmt::Write` is infallible.
        cmd.write_command_sections(&mut menu, categories, width).ok();
        let menu = menu.ansi().to_string();

        let mut args_only = cmd
            .clone()
            .disable_help_subcommand(true)
            .mut_subcommands(|subcommand| subcommand.hide(true))
            .help_template("{all-args}")
            .with_rendered_arg_help();
        let headings = Headings { cmd, messages };
        let short_args = headings.reword(&args_only.clone().render_help());
        let long_args = headings.reword(&args_only.render_long_help());

        let after_help = cmd.get_after_help().map(|text| text.ansi().to_string());
        let after_long_help = cmd.get_after_long_help().map(|text| text.ansi().to_string());
        let short = Self::paragraphs([&menu, &short_args, after_help.as_deref().unwrap_or("")]);
        let long = cmd.has_long_help().then(|| {
            let after = after_long_help.or(after_help);
            Self::paragraphs([&menu, &long_args, after.as_deref().unwrap_or("")])
        });
        Self { long, short }
    }

    /// `parts` that aren't empty, a blank line apart.
    fn paragraphs<const N: usize>(parts: [&str; N]) -> String {
        parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join("\n\n")
    }
}

impl Headings<'_> {
    /// `rendered`, clap's `{all-args}`, with its default section headings reworded, its trailing
    /// newline dropped.
    fn reword(&self, rendered: &StyledStr) -> String {
        let header = self.cmd.get_styles().get_header();
        let styled = |heading: &str| format!("{header}{heading}{header:#}\n");
        let mut text = rendered.ansi().to_string().trim_end().to_owned();
        let defaults = Messages::DEFAULT;
        for (default, worded) in [
            (defaults.arguments.get(), self.messages.arguments.get()),
            (defaults.options.get(), self.messages.options.get()),
        ] {
            if default == worded {
                continue;
            }
            let (clap_heading, couture_heading) = (styled(&default), styled(&worded));
            // A section starts the text or follows a blank line; clap writes the arguments first.
            let at = if text.starts_with(&clap_heading) {
                Some(0)
            } else {
                text.find(&format!("\n\n{clap_heading}")).map(|blank| blank.saturating_add(2))
            };
            if let Some(at) = at {
                text.replace_range(at..at.saturating_add(clap_heading.len()), &couture_heading);
            }
        }
        text
    }
}

impl HelpTemplates for Command {
    fn couture_template(&self, messages: &Messages) -> String {
        let usage = self.get_styles().get_usage();
        let usage_heading = messages.usage.get();
        // clap's `{usage-heading}` is always English, so the heading is written out.
        format!(
            "{{before-help}}{{about-with-newline}}\n{usage}{usage_heading}{usage:#} \
             {{usage}}{{after-help}}"
        )
    }

    fn has_long_help(&self) -> bool {
        let shows_more_long = |arg: &Arg| {
            !arg.is_hide_set()
                && (arg.get_long_help().is_some()
                    || arg.is_hide_long_help_set()
                    || arg.is_hide_short_help_set()
                    || !arg.is_hide_possible_values_set()
                        && arg
                            .get_possible_values()
                            .iter()
                            .any(|value| !value.is_hide_set() && value.get_help().is_some()))
        };
        self.get_long_about().is_some()
            || self.get_before_long_help().is_some()
            || self.get_after_long_help().is_some()
            || self.get_arguments().any(shows_more_long)
    }

    fn has_own_template(&self) -> bool {
        let rendered = self.clone().render_help().ansi().to_string();
        Self::CLAP_DEFAULTS.iter().all(|template| {
            self.clone().help_template(*template).render_help().ansi().to_string() != rendered
        })
    }
}
