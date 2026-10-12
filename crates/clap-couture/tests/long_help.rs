//! couture's `--help` shows more than its `-h` exactly when clap's does.

use core::iter;

use clap::{Arg, ArgAction, Command, Parser, builder::PossibleValue, error::ErrorKind};
use clap_couture::{CommandExt as _, Couture, CoutureParser as _, ParseError};
use rstest::rstest;

/// Where the arg under test is declared.
#[derive(Clone, Copy, Debug)]
enum ArgHome {
    /// Global on `app`, which may be the command under test.
    AppGlobal,
    /// On the command under test.
    UnderTest,
}

/// What the arg under test declares.
#[derive(Clone, Copy, Debug)]
enum ArgShape {
    Flag,
    HiddenWithLongHelp,
    HideLongHelp,
    HideShortHelp,
    LongHelp,
    None,
    ValuesWithHelp,
    ValuesWithHelpHidden,
    ValuesWithHelpHiddenByArg,
    ValuesWithHelpOnHiddenArg,
    ValuesWithoutHelp,
}

/// A command's own text for `--help` alone.
#[derive(Clone, Copy, Debug)]
enum CommandLongText {
    AfterLongHelp,
    BeforeLongHelp,
    LongAbout,
    None,
}

/// Where a command's `hide_possible_values` is set.
#[derive(Clone, Copy, Debug)]
enum HiddenPossibleValues {
    /// On the parent, which sets it for the commands below too.
    Inherited,
    None,
    Own,
}

/// A command `app` with a subcommand `sub` with a subcommand `leaf`, the settings under test on
/// `app` or `sub`.
#[derive(Clone, Copy, Debug)]
struct Shape {
    arg: ArgShape,
    arg_home: ArgHome,
    disable_help_flag: bool,
    disable_help_subcommand: bool,
    hidden_possible_values: HiddenPossibleValues,
    long_text: CommandLongText,
    /// The command below the one under test has a long about.
    subcommand_long_about: bool,
    under_test: UnderTest,
}

/// No categories, so couture's help is clap's.
struct Uncategorized;

/// The command the settings are on.
#[derive(Clone, Copy, Debug)]
enum UnderTest {
    App,
    Sub,
}

#[derive(Parser, Couture)]
#[command(name = "copy", disable_help_flag = true, hide_possible_values = true)]
struct Hiding {
    #[arg(long, action = ArgAction::HelpLong)]
    help: Option<bool>,

    /// How to copy.
    #[arg(long, value_parser = [PossibleValue::new("fast").help("Skip the checks")])]
    mode: Option<String>,
}

impl ArgShape {
    const ALL: [Self; 11] = [
        Self::Flag,
        Self::HiddenWithLongHelp,
        Self::HideLongHelp,
        Self::HideShortHelp,
        Self::LongHelp,
        Self::None,
        Self::ValuesWithHelp,
        Self::ValuesWithHelpHidden,
        Self::ValuesWithHelpHiddenByArg,
        Self::ValuesWithHelpOnHiddenArg,
        Self::ValuesWithoutHelp,
    ];

    /// The arg; `None` for [`Self::None`].
    fn arg(self) -> Option<Arg> {
        let flag = Arg::new("force").long("force").action(ArgAction::SetTrue).help("Overwrite");
        let values = |value: PossibleValue| {
            Arg::new("mode").long("mode").help("How to copy").value_parser([value])
        };
        let with_help = PossibleValue::new("fast").help("Skip the checks");
        Some(match self {
            Self::Flag => flag,
            Self::HiddenWithLongHelp => flag.long_help("Even read-only files").hide(true),
            Self::HideLongHelp => flag.hide_long_help(true),
            Self::HideShortHelp => flag.hide_short_help(true),
            Self::LongHelp => flag.long_help("Even read-only files"),
            Self::None => return None,
            Self::ValuesWithHelp => values(with_help),
            Self::ValuesWithHelpHidden => values(with_help.hide(true)),
            Self::ValuesWithHelpHiddenByArg => values(with_help).hide_possible_values(true),
            Self::ValuesWithHelpOnHiddenArg => values(with_help).hide(true),
            Self::ValuesWithoutHelp => values(PossibleValue::new("fast")),
        })
    }
}

impl CommandLongText {
    const ALL: [Self; 4] = [Self::AfterLongHelp, Self::BeforeLongHelp, Self::LongAbout, Self::None];

    /// `cmd` with this text.
    fn on(self, cmd: Command) -> Command {
        match self {
            Self::AfterLongHelp => cmd.after_long_help("See the manual."),
            Self::BeforeLongHelp => cmd.before_long_help("Copies files."),
            Self::LongAbout => cmd.about("Copy").long_about("Copy files, every one."),
            Self::None => cmd,
        }
    }
}

impl HiddenPossibleValues {
    const ALL: [Self; 3] = [Self::Inherited, Self::None, Self::Own];
}

impl Shape {
    /// A shape with nothing set, on `app`.
    const PLAIN: Self = Self {
        arg: ArgShape::None,
        arg_home: ArgHome::UnderTest,
        disable_help_flag: false,
        disable_help_subcommand: false,
        hidden_possible_values: HiddenPossibleValues::None,
        long_text: CommandLongText::None,
        subcommand_long_about: false,
        under_test: UnderTest::App,
    };

    /// Every shape: each setting with each other, on `app` and on `sub`.
    fn all() -> Vec<Self> {
        let bools = [false, true];
        let mut shapes = vec![Self::PLAIN];
        Self::vary(&mut shapes, &ArgShape::ALL, |shape, arg| shape.arg = arg);
        Self::vary(&mut shapes, &[ArgHome::AppGlobal, ArgHome::UnderTest], |shape, home| {
            shape.arg_home = home;
        });
        Self::vary(&mut shapes, &bools, |shape, yes| shape.disable_help_flag = yes);
        Self::vary(&mut shapes, &bools, |shape, yes| shape.disable_help_subcommand = yes);
        Self::vary(&mut shapes, &HiddenPossibleValues::ALL, |shape, hidden| {
            shape.hidden_possible_values = hidden;
        });
        Self::vary(&mut shapes, &CommandLongText::ALL, |shape, text| shape.long_text = text);
        Self::vary(&mut shapes, &bools, |shape, yes| shape.subcommand_long_about = yes);
        Self::vary(&mut shapes, &[UnderTest::App, UnderTest::Sub], |shape, under_test| {
            shape.under_test = under_test;
        });
        // Only `sub` has a parent to inherit from.
        shapes.retain(|shape| {
            matches!(shape.under_test, UnderTest::Sub)
                || !matches!(shape.hidden_possible_values, HiddenPossibleValues::Inherited)
        });
        shapes
    }

    /// The help argvs: `-h`, `--help`, and `help`, before and after the command's name.
    fn argvs(self) -> Vec<Vec<&'static str>> {
        let path: &[&str] = match self.under_test {
            UnderTest::App => &[],
            UnderTest::Sub => &["sub"],
        };
        let mut argvs: Vec<Vec<&str>> = ["-h", "--help", "help"]
            .map(|last| iter::once("app").chain(path.iter().copied()).chain([last]).collect())
            .into();
        if matches!(self.under_test, UnderTest::Sub) {
            argvs.push(vec!["app", "help", "sub"]);
        }
        argvs
    }

    /// The root command. Its subcommands are hidden, so couture lists none clap doesn't.
    fn root(self) -> Command {
        let long_about = |cmd: Command| {
            if self.subcommand_long_about { cmd.long_about("Below, in full.") } else { cmd }
        };
        let sub = Command::new("sub").hide(true).subcommand(Command::new("leaf").hide(true));
        let app = match self.under_test {
            UnderTest::App => self.with_settings(Command::new("app").subcommand(long_about(sub))),
            UnderTest::Sub => Command::new("app")
                .subcommand(self.with_settings(sub.mut_subcommand("leaf", long_about))),
        };
        let app = match (self.arg_home, self.arg.arg()) {
            (ArgHome::AppGlobal, Some(arg)) => app.arg(arg.global(true)),
            _ => app,
        };
        if matches!(self.hidden_possible_values, HiddenPossibleValues::Inherited) {
            app.hide_possible_values(true)
        } else {
            app
        }
    }

    /// Each shape in `shapes` once with each of `values`, `set` on it.
    fn vary<T, F>(shapes: &mut Vec<Self>, values: &[T], set: F)
    where
        T: Copy,
        F: Fn(&mut Self, T),
    {
        *shapes = shapes
            .iter()
            .flat_map(|shape| {
                values.iter().map(|value| {
                    let mut varied = *shape;
                    set(&mut varied, *value);
                    varied
                })
            })
            .collect();
    }

    /// `cmd` with the settings under test.
    fn with_settings(self, cmd: Command) -> Command {
        let cmd = self.long_text.on(cmd).disable_help_subcommand(self.disable_help_subcommand);
        let cmd = if matches!(self.hidden_possible_values, HiddenPossibleValues::Own) {
            cmd.hide_possible_values(true)
        } else {
            cmd
        };
        let cmd = match (self.arg_home, self.arg.arg()) {
            (ArgHome::UnderTest, Some(arg)) => cmd.arg(arg),
            _ => cmd,
        };
        if self.disable_help_flag {
            // Without clap's help flag, the app asks for each help with its own.
            cmd.disable_help_flag(true)
                .arg(Arg::new("summary").short('h').action(ArgAction::HelpShort))
                .arg(Arg::new("full").long("help").action(ArgAction::HelpLong))
        } else {
            cmd
        }
    }
}

impl Couture for Uncategorized {}

/// What `argv` prints when it asks `cmd` for help; `None` when it prints none.
fn displayed_help(cmd: &Command, argv: &[&str]) -> Option<String> {
    let err = cmd.clone().try_get_matches_from(argv).err()?;
    (err.kind() == ErrorKind::DisplayHelp).then(|| err.render().to_string())
}

#[rstest]
fn a_command_s_hidden_possible_values_still_count_without_a_help_flag() {
    let mode = PossibleValue::new("fast").help("Skip the checks");
    let cmd = Command::new("copy")
        .disable_help_flag(true)
        .hide_possible_values(true)
        .arg(Arg::new("help").long("help").action(ArgAction::HelpLong))
        .arg(Arg::new("mode").long("mode").help("How to copy").value_parser([mode]));
    let clap = displayed_help(&cmd, &["copy", "--help"]);
    // clap decides before it hides the command's possible values, so `--help` shows more.
    let long_mode = "--mode <mode>\n          How to copy\n";
    assert!(clap.as_ref().is_some_and(|help| help.contains(long_mode)), "{clap:?}");
    let couture = cmd.with_couture::<Uncategorized>();
    assert_eq!(displayed_help(&couture, &["copy", "--help"]), clap);
}

#[rstest]
fn a_parser_s_hidden_possible_values_still_count_without_a_help_flag() {
    let clap = displayed_help(&<Hiding as clap::CommandFactory>::command(), &["copy", "--help"]);
    let parsed = match Hiding::couture_try_parse_from(["copy", "--help"]) {
        Err(ParseError::Clap(err)) => Some(err.render().to_string()),
        other => Some(format!("{:?}", other.err())),
    };
    assert_eq!(parsed, clap);
}

#[rstest]
fn each_help_shows_what_clap_s_does() {
    let mut compared = 0usize;
    for shape in Shape::all() {
        let root = shape.root();
        let couture = root.clone().with_couture::<Uncategorized>();
        for argv in shape.argvs() {
            let clap = displayed_help(&root, &argv);
            assert_eq!(displayed_help(&couture, &argv), clap, "for {shape:?} running {argv:?}");
            compared = compared.saturating_add(usize::from(clap.is_some()));
        }
    }
    assert!(compared > 4000, "only {compared} helps were compared");
}
