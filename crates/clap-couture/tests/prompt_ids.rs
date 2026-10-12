//! Every `#[couture(prompt)]` must name an arg clap defines at the same subcommand path, or the
//! interactive parse can never find it. clap is the reference.
#![cfg(feature = "interactive")]

use clap::{Args, Command, CommandFactory as _, Parser, Subcommand};
use clap_couture::Couture;
use rstest::rstest;

#[derive(Parser, Couture)]
#[command(rename_all = "SCREAMING_SNAKE_CASE")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    #[command(flatten)]
    common: Common,

    // clap still ids an arg by the `name` that `id` replaced.
    #[arg(long, name = "legacy")]
    #[couture(prompt)]
    old: String,

    // clap ids an arg by its field, whatever its long and value names.
    #[arg(long = "out-dir", value_name = "DIR")]
    #[couture(prompt)]
    output: String,

    #[arg(long, id = "zone")]
    #[couture(prompt)]
    region: String,

    #[arg(long)]
    #[couture(prompt)]
    r#type: String,
}

#[derive(Args, Couture)]
#[command(rename_all = "UPPER")]
struct Common {
    #[arg(long)]
    #[couture(prompt)]
    team_name: String,
}

#[derive(Subcommand, Couture)]
#[command(rename_all = "snake_case")]
enum Cmd {
    #[command(name = "up")]
    Deploy(DeployArgs),
    DryRun {
        #[arg(long)]
        #[couture(prompt)]
        plan: String,
    },
    #[command(flatten)]
    Extra(Extra),
    #[command(subcommand)]
    Remote(Remote),
}

#[derive(Subcommand, Couture)]
enum Extra {
    #[command(name = "first", name = "gc")]
    Collect {
        #[arg(long)]
        #[couture(prompt)]
        depth: String,
    },
}

#[derive(Args, Couture)]
struct DeployArgs {
    #[arg(long)]
    #[couture(prompt)]
    tag: String,
}

#[derive(Subcommand, Couture)]
#[command(rename_all = "lower")]
enum Remote {
    #[command(rename_all = "camelCase")]
    PullAll {
        #[arg(long)]
        #[couture(prompt)]
        branch: String,
    },
    Push {
        #[arg(long)]
        #[couture(prompt)]
        force_name: String,
    },
}

fn arg_at<'cmd>(cmd: &'cmd Command, path: &[&str], id: &str) -> Option<&'cmd clap::Arg> {
    let level = path.iter().try_fold(cmd, |level, name| level.find_subcommand(name))?;
    level.get_arguments().find(|arg| arg.get_id() == id)
}

#[rstest]
fn prompted_args_follow_the_command_tree() {
    let prompted_args: Vec<_> = Cli::PROMPTED_ARGS
        .prompted_args()
        .into_iter()
        .map(|prompted_arg| (prompted_arg.subcommand_path.join(" "), prompted_arg.attr.arg_id))
        .collect();
    let expected = [
        (String::new(), "legacy"),
        (String::new(), "output"),
        (String::new(), "zone"),
        (String::new(), "type"),
        ("up".to_owned(), "tag"),
        ("dry_run".to_owned(), "plan"),
        ("gc".to_owned(), "depth"),
        ("remote pullAll".to_owned(), "branch"),
        ("remote push".to_owned(), "force_name"),
        (String::new(), "team_name"),
    ];
    assert_eq!(prompted_args, expected);
}

#[rstest]
fn every_prompted_arg_is_an_arg_clap_defines() {
    let cmd = Cli::command();
    let missing: Vec<_> = Cli::PROMPTED_ARGS
        .prompted_args()
        .into_iter()
        .filter(|prompted_arg| {
            arg_at(&cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id).is_none()
        })
        .collect();
    assert!(missing.is_empty(), "clap defines no arg for {missing:?}");
}
