//! couture's copies of clap's help internals, checked against the clap actually built over a
//! generated corpus: a clap release that changes what couture copied fails here, not as help
//! printed twice or a `--help` that shows the short text.

use core::{iter, slice};

use clap::{Arg, ArgAction, Command, builder::StyledStr, error::ErrorKind};
use rstest::rstest;
use unicode_width::UnicodeWidthStr as _;

use super::{CommandRenderExt as _, help::HelpTemplates as _};
use crate::{
    Category, CommandCategoryMap, CommandExt as _, CommandName, Couture,
    command_shapes::CommandShapes,
};

/// How many root commands each check generates.
const SHAPES: u64 = 400;

/// clap's help split around the section listing a command's subcommands.
struct CommandsSection<'help> {
    /// From the blank line below the rows; a newline when nothing follows them.
    after: &'help str,
    /// Up to and including the blank line above the heading.
    before: &'help str,
    /// The rows under the heading.
    rows: &'help str,
}

/// Files some generated subcommands under two categories, leaving the rest uncategorized.
struct Filed;

/// A generated root command, and the seed that built it.
struct Shape {
    root: Command,
    seed: u64,
}

/// No categories, so couture lists every subcommand clap does.
struct Uncategorized;

impl<'help> CommandsSection<'help> {
    /// `help`'s section listing built `level`'s subcommands; `None` when it lists none.
    fn of(help: &'help str, level: &Command) -> Option<Self> {
        let header = level.get_styles().get_header();
        let heading = level.get_subcommand_help_heading().unwrap_or("Commands");
        let heading = format!("\n\n{header}{heading}:{header:#}\n");
        let start = lists_subcommands(level).then(|| help.find(&heading)).flatten()?;
        let before = help.get(..start.saturating_add(2))?;
        let rest = help.get(start.saturating_add(heading.len())..)?;
        let (rows, after) = match rest.find("\n\n") {
            Some(end) => (rest.get(..end)?, rest.get(end..)?),
            None => (rest.strip_suffix('\n').unwrap_or(rest), "\n"),
        };
        Some(Self { after, before, rows })
    }
}

impl Couture for Filed {
    const CATEGORIES: CommandCategoryMap = CommandCategoryMap::new(&[
        (CommandName::new("build"), Category::new("Ship")),
        (CommandName::new("deploy"), Category::new("Ship")),
        (CommandName::new("status"), Category::new("Look")),
        (CommandName::new("ls"), Category::new("Look")),
    ])
    .with_declared_categories(&[Category::new("Look")]);
}

impl Filed {
    /// Check whether `line` is one of the category headings, styled or not.
    fn is_heading(line: &str) -> bool {
        let visible = super::Word(line).visible();
        visible == "Ship:" || visible == "Look:"
    }
}

impl Shape {
    /// The argvs that ask built `root` for the help of its command at `path`: `-h` and `--help`
    /// after it, `help` before it, and `help` before its last name.
    fn argvs(root: &Command, path: &[String]) -> Vec<Vec<String>> {
        let words = |words: &[&[String]]| -> Vec<String> {
            iter::once("app".to_owned())
                .chain(words.iter().flat_map(|part| part.iter().cloned()))
                .collect()
        };
        let help = ["help".to_owned()];
        let mut argvs: Vec<Vec<String>> =
            ["-h", "--help"].map(|flag| words(&[path, &[flag.to_owned()]])).into();
        if let Some((last, parents)) = path.split_last() {
            if has_help_subcommand(root) {
                argvs.push(words(&[&help, path]));
            }
            if !parents.is_empty() && command_at(root, parents).is_some_and(has_help_subcommand) {
                argvs.push(words(&[parents, &help, slice::from_ref(last)]));
            }
        }
        argvs
    }

    /// The built root.
    fn built(&self) -> Command {
        let mut built = self.root.clone();
        built.build();
        built
    }

    /// The generated corpus.
    fn corpus() -> impl Iterator<Item = Self> {
        (0..SHAPES).map(|seed| Self { root: CommandShapes::new(seed).root(), seed })
    }

    /// The root built as couture decides long help on.
    fn deciding(&self) -> Command {
        let mut deciding = self.root.clone().without_command_hide_possible_values();
        deciding.build();
        deciding
    }
}

impl Couture for Uncategorized {}

/// Whether clap's `-h` on the command at `path` below `root` points to `--help`, which it does
/// exactly when `--help` shows more; a command without clap's help flag is given it back to tell.
fn clap_shows_more_for_long_help(root: &Command, path: &[String]) -> Option<bool> {
    let mut built = with_help_flag_at(root.clone(), path);
    built.build();
    let help_flag = command_at(&built, path)?
        .get_arguments()
        .find(|arg| arg.get_id() == "help" && matches!(arg.get_action(), ArgAction::Help))?;
    Some(help_flag.get_help().is_some_and(|help| help.to_string().contains("(see more with")))
}

/// The command at `path` below `cmd`.
fn command_at<'cmd>(cmd: &'cmd Command, path: &[String]) -> Option<&'cmd Command> {
    path.iter().try_fold(cmd, |level, name| level.find_subcommand(name))
}

/// Check whether no command above `path` in `root` declares a global arg.
fn declares_no_global_args_above(root: &Command, path: &[String]) -> bool {
    (0..path.len()).all(|depth| {
        let above = path.get(..depth).and_then(|above| command_at(root, above));
        above.is_some_and(|above| !above.get_arguments().any(Arg::is_global_set))
    })
}

/// What `argv` prints when it asks for help; `None` when it prints none.
fn displayed_help(cmd: &Command, argv: &[String]) -> Option<StyledStr> {
    let err = cmd.clone().try_get_matches_from(argv).err()?;
    (err.kind() == ErrorKind::DisplayHelp).then(|| err.render())
}

/// Check whether every line of `help` below its usage fits `width`. clap wraps couture's body as
/// it wraps an after-help, so a line clap itself lets overflow, like a long arg's flags, breaks
/// there.
fn fits(help: &StyledStr, width: Option<usize>) -> bool {
    let help = help.to_string();
    let below_usage = help
        .find("Usage:")
        .and_then(|usage| help.get(usage..))
        .and_then(|usage| usage.split_once("\n\n"))
        .map_or(help.as_str(), |(_, below)| below);
    width.is_none_or(|width| below_usage.lines().all(|line| line.width() <= width))
}

/// Check whether clap generated a `help` subcommand for built `cmd`.
fn has_help_subcommand(cmd: &Command) -> bool {
    !cmd.is_disable_help_subcommand_set() && cmd.find_subcommand("help").is_some()
}

/// Check whether clap's help of built `level` lists its subcommands.
fn lists_subcommands(level: &Command) -> bool {
    !level.is_flatten_help_set()
        && level.get_subcommands().any(|sub| sub.get_name() != "help" && !sub.is_hide_set())
}

/// The subcommand path to each command below `cmd`, `cmd`'s own empty one first, past the `help`
/// subcommand clap generates.
fn paths(cmd: &Command) -> Vec<Vec<String>> {
    let mut all = vec![Vec::new()];
    let generated_help = |sub: &Command| sub.get_name() == "help" && has_help_subcommand(cmd);
    for sub in cmd.get_subcommands().filter(|sub| !generated_help(sub)) {
        for mut path in paths(sub) {
            path.insert(0, sub.get_name().to_owned());
            all.push(path);
        }
    }
    all
}

/// `cmd` with clap's help flag on the command at `path`.
fn with_help_flag_at(cmd: Command, path: &[String]) -> Command {
    match path.split_first() {
        Some((name, below)) => cmd.mut_subcommand(name, |sub| with_help_flag_at(sub, below)),
        None => cmd.disable_help_flag(false),
    }
}

/// `help` styled, with each style still open at a line's end closed there and opened again past
/// the next line's indent, as couture's markdown shows the subcommands.
fn styles_reopened(help: &StyledStr) -> String {
    let ansi = help.ansi().to_string();
    if cfg!(feature = "markdown") { super::Word(&ansi).with_styles_reopened() } else { ansi }
}

/// clap's `help` of built `level` without the heading over its subcommands, which couture leaves
/// out.
fn without_commands_heading(help: &str, level: &Command) -> String {
    CommandsSection::of(help, level).map_or_else(
        || help.to_owned(),
        |section| format!("{}{}{}", section.before, section.rows, section.after),
    )
}

#[rstest]
fn categories_only_regroup_clap_s_rows() {
    let mut compared = 0usize;
    for shape in Shape::corpus() {
        let couture = shape.root.clone().with_couture::<Filed>();
        let built = shape.built();
        let width = built.help_width();
        for flag in ["-h", "--help"] {
            let argv = ["app", flag].map(str::to_owned);
            let (Some(clap), Some(ours)) =
                (displayed_help(&shape.root, &argv), displayed_help(&couture, &argv))
            else {
                continue;
            };
            if !fits(&clap, width) {
                continue;
            }
            let (clap, ours) = (styles_reopened(&clap), styles_reopened(&ours));
            let Some(section) = CommandsSection::of(&clap, &built) else {
                assert_eq!(ours, clap, "for seed {} running {argv:?}", shape.seed);
                continue;
            };
            // Everything around the menu is clap's, and the menu holds clap's rows.
            let menu =
                ours.strip_prefix(section.before).and_then(|ours| ours.strip_suffix(section.after));
            assert!(menu.is_some(), "for seed {} running {argv:?}:\n{ours}\n{clap}", shape.seed);
            let mut menu_lines: Vec<&str> = menu
                .unwrap_or_default()
                .lines()
                .filter(|line| !line.is_empty() && !Filed::is_heading(line))
                .collect();
            let mut clap_lines: Vec<&str> = section.rows.lines().collect();
            menu_lines.sort_unstable();
            clap_lines.sort_unstable();
            assert_eq!(menu_lines, clap_lines, "for seed {} running {argv:?}", shape.seed);
            compared = compared.saturating_add(1);
        }
    }
    assert!(compared > 200, "only {compared} menus were compared");
}

#[rstest]
fn clap_renders_every_command_under_a_template_couture_copied() {
    for shape in Shape::corpus() {
        for root in [shape.root.clone(), shape.built()] {
            for path in paths(&root) {
                let Some(level) = command_at(&root, &path) else { continue };
                assert!(
                    !level.has_own_template(),
                    "clap's default template is none couture copied, for seed {} at {path:?}:\n{}",
                    shape.seed,
                    level.clone().render_help().ansi(),
                );
            }
        }
    }
}

#[rstest]
fn long_help_exists_where_clap_says_so() {
    let (mut built_checked, mut declared_checked) = (0usize, 0usize);
    for shape in Shape::corpus() {
        let deciding = shape.deciding();
        for path in paths(&deciding) {
            let Some(level) = command_at(&deciding, &path) else { continue };
            let Some(clap_says) = clap_shows_more_for_long_help(&shape.root, &path) else {
                continue;
            };
            assert_eq!(level.has_long_help(), clap_says, "for seed {} at {path:?}", shape.seed);
            built_checked = built_checked.saturating_add(1);
            // As declared, before clap's build adds a help flag that holds its answer; only where
            // the build adds no global args from above.
            if declares_no_global_args_above(&shape.root, &path)
                && let Some(declared) = command_at(&shape.root, &path)
            {
                assert_eq!(
                    declared.has_long_help(),
                    clap_says,
                    "as declared, for seed {} at {path:?}",
                    shape.seed
                );
                declared_checked = declared_checked.saturating_add(1);
            }
        }
    }
    assert!(built_checked > 1000, "only {built_checked} built commands were checked");
    assert!(declared_checked > 500, "only {declared_checked} declared commands were checked");
}

#[rstest]
fn the_width_probe_finds_the_width_clap_wraps_to(
    #[values(None, Some(0), Some(30), Some(80), Some(1000))] max_term_width: Option<usize>,
) {
    for term_width in (0..=200).chain([usize::MAX]) {
        let cmd = Command::new("app").term_width(term_width);
        let cmd = match max_term_width {
            Some(max) => cmd.max_term_width(max),
            None => cmd,
        };
        let mut built = cmd.subcommand(Command::new("sub").subcommand(Command::new("leaf")));
        built.build();
        // An explicit width wins over the max, zero turns wrapping off, and as no word shares a
        // line under 3 cells, clap wraps to 1 cell as to 2.
        let expected = (term_width != 0 && term_width != usize::MAX).then_some(term_width.max(2));
        for path in paths(&built) {
            assert_eq!(
                command_at(&built, &path).and_then(Command::help_width),
                expected,
                "term_width {term_width}, max_term_width {max_term_width:?}, at {path:?}"
            );
        }
    }
}

#[rstest]
fn uncategorized_help_is_clap_s() {
    let mut compared = 0usize;
    for shape in Shape::corpus() {
        let couture = shape.root.clone().with_couture::<Uncategorized>();
        let built = shape.built();
        let width = built.help_width();
        for path in paths(&built) {
            let Some(level) = command_at(&built, &path) else { continue };
            for argv in Shape::argvs(&built, &path) {
                let clap = displayed_help(&shape.root, &argv);
                if clap.as_ref().is_some_and(|help| !fits(help, width)) {
                    continue;
                }
                let clap =
                    clap.map(|help| without_commands_heading(&styles_reopened(&help), level));
                let ours = displayed_help(&couture, &argv).map(|help| styles_reopened(&help));
                assert_eq!(ours, clap, "for seed {} running {argv:?}", shape.seed);
                compared = compared.saturating_add(usize::from(clap.is_some()));
            }
        }
    }
    assert!(compared > 2000, "only {compared} helps were compared");
}
