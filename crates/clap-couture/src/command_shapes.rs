//! Generated clap commands, for checking couture's copies of clap against the clap it is built
//! with: every shape is a command clap accepts, and the same seed always builds the same one.

use clap::{
    Arg, ArgAction, Command,
    builder::{
        PossibleValue, Styles,
        styling::{AnsiColor, Effects},
    },
};

/// Whether help text is markdown, which reads some text differently from clap.
const MARKDOWN: bool = cfg!(feature = "markdown");

/// The args one depth of the tree may declare, so a global arg never clashes with one below it.
struct ArgSlots {
    /// An env var name nothing sets.
    env: &'static str,
    /// Unique ids, also used as longs.
    ids: [&'static str; Self::COUNT],
    /// Unique shorts, none of them `h` or `V`.
    shorts: [char; Self::COUNT],
}

/// A deterministic stream of choices: Knuth's MMIX linear congruential generator.
pub(crate) struct Choices(u64);

/// Builds random but reproducible command trees.
pub(crate) struct CommandShapes {
    choices: Choices,
}

impl ArgSlots {
    const COUNT: usize = 11;
    /// One table per depth, the root's first.
    const DEPTHS: [Self; 4] = [
        Self {
            env: "CLAP_COUTURE_SHAPES_COLOR",
            ids: [
                "force", "verbose", "output", "mode", "jobs", "color", "include", "quiet",
                "source", "files", "dry-run",
            ],
            shorts: ['f', 'v', 'o', 'm', 'j', 'c', 'i', 'q', 's', 'F', 'n'],
        },
        Self {
            env: "CLAP_COUTURE_SHAPES_TOKEN",
            ids: [
                "all", "debug", "target", "profile", "level", "token", "feature", "silent", "path",
                "rest", "keep",
            ],
            shorts: ['a', 'd', 't', 'p', 'l', 'k', 'e', 'u', 'w', 'W', 'K'],
        },
        Self {
            env: "CLAP_COUTURE_SHAPES_REGION",
            ids: [
                "yes", "trace", "into", "format", "depth", "region", "tag", "mute", "name",
                "extra", "watch",
            ],
            shorts: ['y', 'r', 'g', 'x', 'z', 'b', 'G', 'M', 'N', 'E', 'A'],
        },
        Self {
            env: "CLAP_COUTURE_SHAPES_ZONE",
            ids: [
                "now", "loud", "dest", "kind", "retries", "zone", "label", "calm", "item", "more",
                "once",
            ],
            shorts: ['O', 'P', 'Q', 'R', 'T', 'U', 'Y', 'Z', 'H', 'I', 'J'],
        },
    ];
}

impl Choices {
    /// A number below `bound`, or 0 when `bound` is 0.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let high_bits = usize::try_from(self.0 >> 33u32).unwrap_or_default();
        high_bits.checked_rem(bound).unwrap_or_default()
    }

    pub(crate) const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// True once in `odds` times.
    pub(crate) fn one_in(&mut self, odds: usize) -> bool {
        self.below(odds) == 0
    }

    /// One of `items`; the default when there are none.
    pub(crate) fn pick<T>(&mut self, items: &[T]) -> T
    where
        T: Copy + Default,
    {
        items.get(self.below(items.len())).copied().unwrap_or_default()
    }
}

impl CommandShapes {
    /// The long aliases the first three slots' args may take, by depth.
    const ALIASES: [[&'static str; 3]; 4] = [
        ["by-force", "chatty", "out"],
        ["every", "dbg", "dest-dir"],
        ["assume-yes", "tracing", "to"],
        ["immediately", "noisy", "target-dir"],
    ];
    /// Headings an arg may go under.
    const HEADINGS: [Option<&'static str>; 3] = [None, Some("Output"), Some("Advanced options")];
    /// Help texts: none, short, ones long enough to wrap, ones with several lines, and wide ones.
    const HELPS: [Option<&'static str>; 6] = [
        None,
        Some("Overwrite"),
        Some("Print each file as it is copied"),
        Some(
            "Read every source in full before writing anything, so a failure part way leaves the \
             destination as it was",
        ),
        // Markdown joins the lines of a paragraph, so it gets two paragraphs instead.
        Some(if MARKDOWN { "Pick one.\n\nOr the other." } else { "Pick one.\nOr the other." }),
        Some("Wide \u{6587}\u{5b57} text"),
    ];
    const LONG_HELPS: [Option<&'static str>; 3] = [
        None,
        Some("Overwrite what is there, even read-only files."),
        Some("At length.\n\nIn two paragraphs, the second of them long enough to wrap."),
    ];
    /// Each subcommand name with the short flag, alias and short flag alias it may take; no two
    /// clash at one level.
    const SUBCOMMANDS: [(&'static str, char, &'static str, char); 6] = [
        ("x", 'X', "ex", '1'),
        ("build", 'B', "b", '2'),
        ("deploy", 'D', "ship", '3'),
        ("status", 'S', "st", '4'),
        ("very-long-subcommand-name", 'L', "vl", '5'),
        ("ls", 'C', "list", '6'),
    ];
    /// Widths clap may be told to wrap to; `None` leaves it to the terminal.
    const TERM_WIDTHS: [Option<usize>; 10] = [
        None,
        Some(0),
        Some(1),
        Some(12),
        Some(24),
        Some(40),
        Some(60),
        Some(80),
        Some(120),
        Some(1000),
    ];

    /// `arg` with random help, visibility, heading and aliases.
    fn arg_settings(&mut self, mut arg: Arg, slot: usize, depth: usize) -> Arg {
        arg = arg.help(self.choices.pick(&Self::HELPS));
        if self.choices.one_in(4) {
            arg = arg.long_help(self.choices.pick(&Self::LONG_HELPS));
        }
        let positional = matches!(slot, 8 | 9);
        arg = arg
            .hide(self.choices.one_in(12))
            .hide_short_help(self.choices.one_in(10))
            .hide_long_help(self.choices.one_in(10))
            .next_line_help(self.choices.one_in(12));
        if self.choices.one_in(5) {
            arg = arg.help_heading(self.choices.pick(&Self::HEADINGS));
        }
        if self.choices.one_in(8) {
            arg = arg.display_order(self.choices.below(5));
        }
        if let Some(alias) = Self::ALIASES.get(depth).and_then(|aliases| aliases.get(slot))
            && self.choices.one_in(4)
        {
            arg =
                if self.choices.one_in(2) { arg.visible_alias(*alias) } else { arg.alias(*alias) };
        }
        if slot == 2 && self.choices.one_in(4) {
            arg = arg.required(true);
        }
        if !positional && depth < 2 && !arg.is_required_set() && self.choices.one_in(5) {
            arg = arg.global(true);
        }
        arg
    }

    /// The args of the command at `depth`: each slot's, with a third of the slots filled.
    fn args(&mut self, mut cmd: Command, depth: usize) -> Command {
        let Some(slots) = ArgSlots::DEPTHS.get(depth) else { return cmd };
        let mut first_positional_required = None;
        for (slot, (id, short)) in slots.ids.iter().zip(slots.shorts).enumerate() {
            if !self.choices.one_in(3) {
                continue;
            }
            if self.choices.one_in(8) {
                cmd = cmd.next_help_heading(self.choices.pick(&Self::HEADINGS));
            }
            let mut arg = Arg::new(*id);
            arg = match slot {
                0 => arg.long(*id).short(short).action(ArgAction::SetTrue),
                1 => arg.short(short).long(*id).action(ArgAction::Count),
                2 => {
                    arg.long(*id).value_name(self.choices.pick(&["PATH", "N", "A_LONG_VALUE_NAME"]))
                }
                3 => self.possible_values(arg.long(*id).short(short)),
                4 => arg
                    .long(*id)
                    .default_value("3")
                    .hide_default_value(self.choices.one_in(4))
                    .value_parser(clap::value_parser!(u8)),
                5 => arg
                    .long(*id)
                    .env(slots.env)
                    .hide_env(self.choices.one_in(4))
                    .hide_env_values(self.choices.one_in(4)),
                6 => match self.choices.below(3) {
                    0 => arg.long(*id).num_args(1..=3),
                    1 => arg.long(*id).action(ArgAction::Append).value_delimiter(','),
                    _ => arg.long(*id).num_args(0..=1).default_missing_value("all"),
                },
                7 => arg.short(short).action(ArgAction::SetTrue),
                8 => {
                    let required = self.choices.one_in(2);
                    first_positional_required = Some(required);
                    arg.required(required)
                }
                9 => {
                    // A required positional after an optional one is one clap refuses.
                    let required =
                        first_positional_required.unwrap_or(true) && self.choices.one_in(3);
                    arg.num_args(1..).required(required).value_name("FILE")
                }
                _ => arg.long(*id).action(ArgAction::SetFalse),
            };
            arg = self.arg_settings(arg, slot, depth);
            cmd = cmd.arg(arg);
        }
        cmd
    }

    /// The command at `depth` named `name`: random args, settings and subcommands below it.
    fn command(&mut self, name: &'static str, depth: usize) -> Command {
        let mut cmd = Command::new(name);
        let (about, long_about) = self.command_texts();
        // Markdown lists a subcommand's about on one line, where clap keeps its lines.
        let listable =
            |text: Option<&'static str>| text.filter(|text| !MARKDOWN || !text.contains('\n'));
        cmd = cmd.about(listable(about)).long_about(listable(long_about));
        let (before, before_long) = self.command_texts();
        cmd = cmd.before_help(before).before_long_help(before_long);
        let (after, after_long) = self.command_texts();
        cmd = cmd.after_help(after).after_long_help(after_long);
        cmd = self.settings(cmd);
        cmd = self.args(cmd, depth);
        if depth < 3 {
            let count = self.choices.pick(&[0usize, 0usize, 1usize, 2usize, 3usize, 3usize]);
            let mut names = Self::SUBCOMMANDS.to_vec();
            for _ in 0usize..count {
                let picked = self.choices.below(names.len());
                if picked < names.len() {
                    let names_and_flags = names.remove(picked);
                    let sub = self.command(names_and_flags.0, depth.saturating_add(1));
                    cmd = cmd.subcommand(self.subcommand_settings(sub, names_and_flags));
                }
            }
        }
        cmd
    }

    /// The about, before or after text of a command: none, short, long, or one under each.
    fn command_texts(&mut self) -> (Option<&'static str>, Option<&'static str>) {
        let short = self.choices.pick(&Self::HELPS);
        let long = self.choices.pick(&Self::LONG_HELPS);
        match self.choices.below(4) {
            0 => (None, None),
            1 => (short, None),
            2 => (None, long),
            _ => (short, long),
        }
    }

    pub(crate) const fn new(seed: u64) -> Self {
        Self { choices: Choices::new(seed) }
    }

    /// `arg` taking a few named values, some with help, one hidden.
    fn possible_values(&mut self, arg: Arg) -> Arg {
        let values = match self.choices.below(3) {
            0 => vec![PossibleValue::new("fast"), PossibleValue::new("safe")],
            1 => vec![
                PossibleValue::new("fast").help("Skip the checks"),
                PossibleValue::new("safe"),
                PossibleValue::new("slow").hide(true),
            ],
            _ => vec![
                PossibleValue::new("fast"),
                PossibleValue::new("safe").help("Check twice").hide(true),
            ],
        };
        arg.value_parser(values).hide_possible_values(self.choices.one_in(5))
    }

    /// The next root command, named `app`.
    pub(crate) fn root(&mut self) -> Command {
        let mut cmd = self.command("app", 0);
        cmd = match self.choices.below(4) {
            0 => cmd,
            1 => cmd.version("1.2.3"),
            2 => cmd.version("1.2.3").long_version("1.2.3 (long)"),
            _ => cmd.version("1.2.3").propagate_version(true),
        };
        cmd = cmd.disable_version_flag(self.choices.one_in(10));
        cmd = match self.choices.pick(&Self::TERM_WIDTHS) {
            Some(width) => cmd.term_width(width),
            None => cmd,
        };
        if self.choices.one_in(3) {
            cmd = cmd.max_term_width(self.choices.pick(&[0usize, 30usize, 80usize, 100usize]));
        }
        let styles = match self.choices.below(3) {
            0 => Styles::plain(),
            1 => Styles::styled(),
            _ => Styles::styled()
                .header(AnsiColor::Green.on_default() | Effects::BOLD)
                .usage(AnsiColor::Cyan.on_default() | Effects::UNDERLINE)
                .literal(AnsiColor::Blue.on_default())
                .placeholder(AnsiColor::Magenta.on_default())
                .context(AnsiColor::Yellow.on_default()),
        };
        cmd.styles(styles)
    }

    /// `cmd` with random help and subcommand settings.
    fn settings(&mut self, cmd: Command) -> Command {
        let cmd = cmd
            .disable_help_flag(self.choices.one_in(8))
            .next_line_help(self.choices.one_in(8))
            .disable_help_subcommand(self.choices.one_in(5))
            .subcommand_required(self.choices.one_in(4))
            .arg_required_else_help(self.choices.one_in(10))
            .args_conflicts_with_subcommands(self.choices.one_in(8))
            .subcommand_negates_reqs(self.choices.one_in(8))
            .hide_possible_values(self.choices.one_in(12))
            .flatten_help(self.choices.one_in(15));
        let cmd = if self.choices.one_in(8) { cmd.subcommand_help_heading("Tasks") } else { cmd };
        if self.choices.one_in(8) { cmd.subcommand_value_name("TASK") } else { cmd }
    }

    /// `sub` with random visibility, aliases, flags and order, as a subcommand.
    fn subcommand_settings(
        &mut self,
        sub: Command,
        (name, short_flag, alias, short_flag_alias): (&'static str, char, &'static str, char),
    ) -> Command {
        let mut sub = sub.hide(self.choices.one_in(6));
        if self.choices.one_in(5) {
            sub = if self.choices.one_in(2) { sub.visible_alias(alias) } else { sub.alias(alias) };
        }
        if self.choices.one_in(8) {
            sub = sub.short_flag(short_flag);
        }
        if self.choices.one_in(8) {
            sub = sub.long_flag(name);
        }
        if self.choices.one_in(10) {
            sub = sub.visible_short_flag_alias(short_flag_alias);
        }
        if self.choices.one_in(12) {
            sub = sub.visible_long_flag_alias(alias);
        }
        if self.choices.one_in(6) {
            sub = sub.display_order(self.choices.below(4));
        }
        sub
    }
}
