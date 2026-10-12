//! Every clap-couture feature in one CLI: categorized, markdown `--help` in your own clap styles,
//! and prompts for whatever the user leaves out, from a picker to a masked token to `$EDITOR`.
//!
//! ```sh
//! cargo run -p clap-couture --example 99-full-example --features markdown -- --help
//! cargo run -p clap-couture --example 99-full-example --features markdown -- deploy
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{
    Parser, Subcommand, ValueEnum,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
use clap_couture::{Couture, CoutureParser as _, interactive::secrecy::SecretString};

// couture draws `--help` and its prompts in the command's clap styles.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Blue.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Yellow.on_default());

/// Deploy and manage your apps from the **terminal**, *fashionably*.
#[derive(Parser, Couture)]
#[command(name = "orbit", bin_name = "orbit", styles = STYLES)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    // Descriptions are markdown, with the `markdown` feature.
    "ship" = { title = "Ship:", description = "Get code to **production**" },
    "config" = { title = "Config:", description = "What your app *knows*" },
    "account" = { title = "Account:", description = "Who **you** are" },
})]
enum Cmd {
    /// Ship the **current** project.
    #[couture(category = "ship")]
    Deploy(Deploy),
    /// Point a domain at your app.
    #[couture(category = "config")]
    Domains,
    /// Manage `KEY=VALUE` variables.
    #[couture(category = "config")]
    Env,
    /// Sign in to your account.
    #[couture(category = "account")]
    Login(Login),
    /// Sign out of *this* machine.
    #[couture(category = "account")]
    Logout,
    // No category: listed first, under no heading.
    /// Tail live logs.
    Logs,
    /// Promote a release to *production*.
    #[couture(category = "ship")]
    Promote,
    /// Roll back to a previous release.
    #[couture(category = "ship")]
    Rollback(Rollback),
    /// Change the `replicas` count.
    #[couture(category = "ship")]
    Scale,
    // `orbit secrets --help` is categorized too, by the categories on `Secrets`.
    /// Manage environment secrets.
    #[couture(category = "config")]
    #[command(subcommand)]
    Secrets(Secrets),
    /// Show recent activity.
    Status,
    /// Show who you're signed in as.
    #[couture(category = "account")]
    Whoami,
}

#[derive(Parser, Couture)]
struct Deploy {
    /// Application name.
    #[arg(long)]
    // Asks the doc comment.
    #[couture(prompt)]
    app: String,

    /// Release notes, in **markdown**.
    #[arg(long)]
    // Written in the user's `$EDITOR`, like `git commit`.
    #[couture(prompt = "Release notes", editor(extension = "md", initial = "## Changes\n"))]
    notes: Option<String>,

    /// Region to deploy to.
    #[arg(long, value_enum)]
    // A `ValueEnum` is picked from a list.
    #[couture(prompt = "Which region?")]
    region: Region,

    /// Replica count.
    #[arg(long, default_value_t = 2)]
    // Pre-filled with the default, and parsed by clap: `abc` gets clap's error, and the question
    // again.
    #[couture(prompt = "How many replicas?")]
    replicas: u16,

    /// Sidecars to run next to the app.
    #[arg(long, value_enum)]
    // A `Vec` of a `ValueEnum` is a multi-select.
    #[couture(prompt = "Which sidecars?")]
    sidecars: Vec<Sidecar>,

    /// Deploy token.
    #[arg(long, env = "ORBIT_TOKEN")]
    // Masked as it's typed, refused on the command line, and `--token -` reads it from stdin.
    #[couture(prompt, secret, stdin)]
    token: SecretString,

    /// Skip the confirmation.
    #[arg(long)]
    // Asked last, naming the answers above.
    #[couture(confirm = "Deploy {app} to {region}?")]
    yes: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Region {
    Apac,
    Eu,
    Us,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Sidecar {
    Logging,
    Metrics,
    Proxy,
}

#[derive(Parser, Couture)]
struct Rollback {
    /// Release to roll back to; `-` asks for it.
    #[arg(long)]
    #[couture(prompt(dash, question = "Roll back to which release?"))]
    to: String,
}

#[derive(Parser, Couture)]
struct Login {
    /// Your email.
    #[arg(long)]
    #[couture(prompt)]
    email: String,

    /// Your password.
    #[arg(long)]
    // Masked as it's typed; `--password -` reads it from stdin, like `docker login
    // --password-stdin`.
    #[couture(prompt, secret, stdin)]
    password: SecretString,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "read" = { title = "Read:" }, "write" = { title = "Write:" } })]
enum Secrets {
    /// List the secrets' names.
    #[couture(category = "read")]
    List,
    /// Remove a secret.
    #[couture(category = "write")]
    Remove {
        /// The secret's name.
        name: String,
    },
    /// Set a secret.
    #[couture(category = "write")]
    Set(SetSecret),
}

#[derive(Parser, Couture)]
struct SetSecret {
    /// The secret's name.
    #[arg(long)]
    #[couture(prompt)]
    name: String,

    /// The secret's value.
    #[arg(long)]
    // An optional secret: left empty, it stays `None`.
    #[couture(prompt, secret)]
    value: Option<SecretString>,
}

fn main() {
    let cli = Cli::couture_parse();

    match cli.cmd {
        Cmd::Deploy(deploy) => {
            // A real deploy hands the token on; the demo only says it arrived.
            let _token: SecretString = deploy.token;
            let sidecars: Vec<_> = deploy.sidecars.iter().map(name).collect();
            println!(
                "deploying {} to {} with {} replicas and [{}]",
                deploy.app,
                name(&deploy.region),
                deploy.replicas,
                sidecars.join(", ")
            );
            if let Some(notes) = deploy.notes {
                println!("{notes}");
            }
        }
        Cmd::Domains => println!("no domains yet"),
        Cmd::Env => println!("no variables yet"),
        Cmd::Login(login) => println!("signed in as {}", login.email),
        Cmd::Logout => println!("signed out"),
        Cmd::Logs => println!("tailing logs"),
        Cmd::Promote => println!("promoted"),
        Cmd::Rollback(rollback) => println!("rolling back to {}", rollback.to),
        Cmd::Scale => println!("scaled"),
        Cmd::Secrets(Secrets::List) => println!("no secrets yet"),
        Cmd::Secrets(Secrets::Remove { name }) => println!("removed {name}"),
        Cmd::Secrets(Secrets::Set(secret)) => match secret.value {
            Some(_) => println!("set {}", secret.name),
            None => println!("left {} empty", secret.name),
        },
        Cmd::Status => println!("all systems go"),
        Cmd::Whoami => println!("not signed in"),
    }
}

/// `value` as the command line spells it: `eu`, not `Eu`.
fn name(value: &impl ValueEnum) -> String {
    value.to_possible_value().map(|value| value.get_name().to_owned()).unwrap_or_default()
}
