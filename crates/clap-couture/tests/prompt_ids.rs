//! Every `#[couture(prompt)]` must name an arg clap defines at the same subcommand path, or the
//! prompt flow can never find it. clap is the reference.
#![cfg(feature = "interactive")]

use clap::{Args, Command, CommandFactory as _, Parser, Subcommand};
use clap_couture::Couture;
use rstest::rstest;

#[derive(Parser, Couture)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    #[command(flatten)]
    common: Common,

    #[arg(long, id = "zone")]
    #[couture(prompt)]
    region: String,

    #[arg(long)]
    #[couture(prompt)]
    r#type: String,
}

#[derive(Args, Couture)]
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
    #[command(subcommand)]
    Remote(Remote),
}

#[derive(Args, Couture)]
struct DeployArgs {
    #[arg(long)]
    #[couture(prompt)]
    tag: String,
}

#[derive(Subcommand, Couture)]
enum Remote {
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
fn marks_follow_the_command_tree() {
    let marks: Vec<_> =
        Cli::PROMPTS.marks().into_iter().map(|mark| (mark.path.join(" "), mark.spec.id)).collect();
    let expected = [
        (String::new(), "zone"),
        (String::new(), "type"),
        ("up".to_owned(), "tag"),
        ("dry_run".to_owned(), "plan"),
        ("remote push".to_owned(), "force_name"),
        (String::new(), "team_name"),
    ];
    assert_eq!(marks, expected);
}

#[rstest]
fn every_mark_names_an_arg_clap_defines() {
    let cmd = Cli::command();
    let missing: Vec<_> = Cli::PROMPTS
        .marks()
        .into_iter()
        .filter(|mark| arg_at(&cmd, &mark.path, mark.spec.id).is_none())
        .collect();
    assert!(missing.is_empty(), "clap defines no arg for {missing:?}");
}
