//! Secret args: a prompted answer reaches clap only as a placeholder, and the command line is
//! refused unless the arg allows it. A value from an env var or an allowed command line is clap's
//! raw value, as for any arg.

use alloc::sync::Arc;
use core::fmt::Write as _;
use std::{
    ffi::OsStr,
    sync::{Mutex, MutexGuard, PoisonError},
};

use clap::{
    Arg, Command,
    builder::{StyledStr, Styles, TypedValueParser},
    error::ErrorKind,
    parser::ValueSource,
};
use secrecy::SecretString;
use unicode_width::UnicodeWidthStr as _;

use super::prompting::{with_arg_at, with_command_at};
use crate::interactive::PromptedArg;

/// What a prompted secret's placeholder starts with. Neither argv nor env vars can hold NUL, so no
/// user input can forge one.
const PLACEHOLDER_PREFIX: &str = "\0couture-secret:";

/// The prompted secret answers of one parse, shared by every `Command` clone that resolves their
/// placeholders, and zeroized when the last one drops.
#[derive(Clone, Default)]
pub(crate) struct Vault(Arc<Mutex<Vec<SecretString>>>);

impl Vault {
    /// The answer `value` stands for, if it is one of this vault's placeholders.
    fn get(&self, value: &str) -> Option<SecretString> {
        let digits = value.strip_prefix(PLACEHOLDER_PREFIX)?;
        // `store` writes bare digits, but `usize::from_str` would also take a leading `+`.
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let index: usize = digits.parse().ok()?;
        self.secrets().get(index).cloned()
    }

    // Nothing panics while holding the lock, so a poisoned one still holds whole answers.
    fn secrets(&self) -> MutexGuard<'_, Vec<SecretString>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keep `secret`, returning the placeholder clap gets in its place.
    pub(crate) fn store(&self, secret: SecretString) -> String {
        let mut secrets = self.secrets();
        let placeholder = format!("{PLACEHOLDER_PREFIX}{}", secrets.len());
        secrets.push(secret);
        placeholder
    }
}

/// A secret arg's value parser: an injected placeholder resolves from the vault, the command line
/// is refused unless allowed, and anything else (an env var) is taken as is.
#[derive(Clone)]
struct SecretValueParser {
    /// The command line may carry this secret: `secret(allow_argv)`.
    allow_argv: bool,
    vault: Vault,
}

impl TypedValueParser for SecretValueParser {
    type Value = SecretString;

    // Only reached by callers that name no source; assume the most exposed one.
    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<SecretString, clap::Error> {
        self.parse_ref_(cmd, arg, value, ValueSource::CommandLine)
    }

    fn parse_ref_(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
        source: ValueSource,
    ) -> Result<SecretString, clap::Error> {
        if source == ValueSource::CommandLine && !self.allow_argv {
            return Err(command_line_refusal(cmd, arg));
        }
        let value =
            value.to_str().ok_or_else(|| clap::Error::new(ErrorKind::InvalidUtf8).with_cmd(cmd))?;
        if !value.starts_with(PLACEHOLDER_PREFIX) {
            return Ok(SecretString::from(value));
        }
        // Only the prompting parse makes placeholders, and only as default values; one from
        // anywhere else, or naming no answer, must not stand in for a secret.
        let answer = (source == ValueSource::DefaultValue).then(|| self.vault.get(value)).flatten();
        answer.ok_or_else(|| unresolved_placeholder(cmd, arg))
    }
}

struct HiddenSecrets<'path> {
    // Each env var that can pass one of them, with that arg's help.
    env_vars: Vec<(String, String)>,
    path: &'path [&'static str],
}

impl HiddenSecrets<'_> {
    fn after_help(
        &self,
        heading: &str,
        styles: &Styles,
        after_help: Option<&StyledStr>,
    ) -> StyledStr {
        let (header, literal) = (styles.get_header(), styles.get_literal());
        let width = self.env_vars.iter().map(|(env, _)| env.width()).max().unwrap_or_default();
        let mut section = format!("{header}{heading}{header:#}");
        // `String`'s `fmt::Write` is infallible.
        for (env, help) in &self.env_vars {
            let pad = " ".repeat(width.saturating_sub(env.width()));
            write!(section, "\n  {literal}{env}{literal:#}{pad}  {help}").ok();
        }
        if let Some(after_help) = after_help {
            write!(section, "\n\n{}", after_help.ansi()).ok();
        }
        StyledStr::from(section)
    }
}

/// Give each secret arg in `prompted_args` a [`SecretValueParser`] on `vault`, and keep its env
/// var's value and any placeholder default out of `--help`.
pub(crate) fn guard(
    cmd: Command,
    prompted_args: &[PromptedArg],
    vault: &Vault,
    environment_heading: &str,
) -> Command {
    let cmd = prompted_args.iter().fold(cmd, |cmd, prompted_arg| {
        let Some(secret_attr) = &prompted_arg.attr.secret else { return cmd };
        let parser =
            SecretValueParser { allow_argv: secret_attr.allow_argv(), vault: vault.clone() };
        with_arg_at(cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id, |arg| {
            arg.value_parser(parser).hide_default_value(true).hide_env_values(true)
        })
    });
    // Help offers no flag the command line can only fail with; the env vars that pass those
    // secret args are listed instead. An arg already hidden is left alone, so guarding twice adds
    // nothing.
    let mut hidden: Vec<HiddenSecrets<'_>> = Vec::new();
    let refused = prompted_args.iter().filter(|prompted_arg| {
        prompted_arg.attr.secret.as_ref().is_some_and(|secret| !secret.allow_argv())
    });
    let cmd = refused.fold(cmd, |cmd, prompted_arg| {
        let path = prompted_arg.subcommand_path.as_slice();
        let id = prompted_arg.attr.arg_id;
        with_command_at(cmd, path, |level| {
            let Some(arg) = level.get_arguments().find(|arg| arg.get_id() == id) else {
                return level;
            };
            if arg.is_hide_set() {
                return level;
            }
            if let Some(env) = arg.get_env() {
                let help = arg.get_help().map(|help| help.ansi().to_string()).unwrap_or_default();
                let env_var = (env.to_string_lossy().into_owned(), help);
                match hidden.iter_mut().find(|level_secrets| level_secrets.path == path) {
                    Some(level_secrets) => level_secrets.env_vars.push(env_var),
                    None => hidden.push(HiddenSecrets { env_vars: vec![env_var], path }),
                }
            }
            level.mut_arg(id, |arg| arg.hide(true))
        })
    });
    hidden.into_iter().fold(cmd, |cmd, level_secrets| {
        with_command_at(cmd, level_secrets.path, |level| {
            let after_help = level_secrets.after_help(
                environment_heading,
                level.get_styles(),
                level.get_after_help(),
            );
            level.after_help(after_help)
        })
    })
}

/// How an error names the secret arg it is about.
fn arg_name(arg: Option<&Arg>) -> String {
    arg.map_or_else(|| "a secret".to_owned(), ToString::to_string)
}

/// The refusal of a secret passed on the command line, pointing at the ways it may come in.
fn command_line_refusal(cmd: &Command, arg: Option<&Arg>) -> clap::Error {
    let env = arg.and_then(Arg::get_env).map(OsStr::to_string_lossy);
    let hint = env.map_or_else(
        || "leave it out to be asked at a terminal".to_owned(),
        |env| format!("set {env} or leave it out to be asked at a terminal"),
    );
    secret_error(
        cmd,
        &format!("{} is a secret and can't be passed on the command line; {hint}", arg_name(arg)),
    )
}

/// `message` with clap's usage and `--help` pointer, as clap formats its own errors.
fn secret_error(cmd: &Command, message: &str) -> clap::Error {
    clap::Error::raw(ErrorKind::ValueValidation, message).format(&mut cmd.clone())
}

/// The error for a placeholder this parse did not inject: stale, or typed in.
fn unresolved_placeholder(cmd: &Command, arg: Option<&Arg>) -> clap::Error {
    secret_error(
        cmd,
        &format!("{} got a secret placeholder with no answer behind it", arg_name(arg)),
    )
}

#[cfg(test)]
mod tests {
    use clap::{Arg, Command, error::ErrorKind};
    use secrecy::{ExposeSecret, SecretString};

    use super::{PLACEHOLDER_PREFIX, Vault, guard};
    use crate::interactive::{PromptAttr, PromptPurpose, PromptedArg, SecretAttr};

    static TOKEN: PromptAttr = PromptAttr {
        arg_id: "token",
        dash: None,
        editor: None,
        purpose: PromptPurpose::Value,
        question: None,
        secret: Some(SecretAttr::new::<SecretString>(false)),
    };

    static SCRIPTED_TOKEN: PromptAttr = PromptAttr {
        arg_id: "token",
        dash: None,
        editor: None,
        purpose: PromptPurpose::Value,
        question: None,
        secret: Some(SecretAttr::new::<SecretString>(true)),
    };

    fn guarded(attr: &'static PromptAttr, vault: &Vault) -> Command {
        let cmd = Command::new("app").arg(Arg::new("token").long("token"));
        guard(cmd, &[PromptedArg { attr, subcommand_path: Vec::new() }], vault, "Environment:")
    }

    /// The rendered error of parsing `argv`, empty when the parse succeeds.
    fn error_message(cmd: Command, argv: &[&str]) -> String {
        cmd.try_get_matches_from(argv)
            .map(drop)
            .map_err(|err| err.to_string())
            .err()
            .unwrap_or_default()
    }

    fn token(cmd: Command, argv: &[&str]) -> Result<String, ErrorKind> {
        let matches = cmd.try_get_matches_from(argv).map_err(|err| err.kind())?;
        Ok(matches
            .get_one::<SecretString>("token")
            .map(|token| token.expose_secret().to_owned())
            .unwrap_or_default())
    }

    #[test]
    fn a_placeholder_resolves_to_its_answer_and_clap_sees_only_the_placeholder() {
        let vault = Vault::default();
        let placeholder = vault.store(SecretString::from("hunter2"));
        let cmd = guarded(&TOKEN, &vault).mut_arg("token", |arg| arg.default_value(placeholder));
        let matches = cmd.try_get_matches_from(["app"]).ok();
        let raw = matches.as_ref().and_then(|m| m.get_raw("token")?.next()?.to_str());
        assert_eq!(raw.map(|raw| raw.starts_with('\0')), Some(true));
        let answer = matches.as_ref().and_then(|m| m.get_one::<SecretString>("token"));
        assert_eq!(answer.map(ExposeSecret::expose_secret), Some("hunter2"));
    }

    #[test]
    fn a_placeholder_stays_out_of_help() {
        let vault = Vault::default();
        let placeholder = vault.store(SecretString::from("hunter2"));
        let mut cmd =
            guarded(&TOKEN, &vault).mut_arg("token", |arg| arg.default_value(placeholder));
        let help = cmd.render_help().to_string();
        assert!(!help.contains("couture-secret"), "{help:?}");
    }

    #[test]
    fn the_command_line_is_refused() {
        let cmd = guarded(&TOKEN, &Vault::default());
        assert_eq!(token(cmd, &["app", "--token", "x"]), Err(ErrorKind::ValueValidation));
    }

    #[test]
    fn the_refusal_reads_like_a_clap_error() {
        let message = error_message(guarded(&TOKEN, &Vault::default()), &["app", "--token", "x"]);
        assert_eq!(
            message,
            "error: --token <token> is a secret and can't be passed on the command line; leave it \
             out to be asked at a terminal\n\nUsage: app\n\nFor more information, try '--help'.\n"
        );
    }

    #[test]
    fn the_refusal_points_at_the_env_var() {
        let cmd = guarded(&TOKEN, &Vault::default()).mut_arg("token", |arg| arg.env("APP_TOKEN"));
        let message = error_message(cmd, &["app", "--token", "x"]);
        assert!(
            message.contains("; set APP_TOKEN or leave it out to be asked at a terminal"),
            "{message}"
        );
    }

    #[test]
    fn allow_argv_takes_the_command_line() {
        let cmd = guarded(&SCRIPTED_TOKEN, &Vault::default());
        assert_eq!(token(cmd, &["app", "--token", "x"]).as_deref(), Ok("x"));
    }

    #[test]
    fn a_placeholder_typed_on_the_command_line_resolves_to_nothing() {
        let vault = Vault::default();
        let placeholder = vault.store(SecretString::from("hunter2"));
        let cmd = guarded(&SCRIPTED_TOKEN, &vault);
        assert_eq!(token(cmd, &["app", "--token", &placeholder]), Err(ErrorKind::ValueValidation));
    }

    #[test]
    fn a_placeholder_from_another_vault_is_an_error() {
        let placeholder = Vault::default().store(SecretString::from("hunter2"));
        let cmd = guarded(&TOKEN, &Vault::default())
            .mut_arg("token", |arg| arg.default_value(placeholder));
        assert_eq!(token(cmd, &["app"]), Err(ErrorKind::ValueValidation));
    }

    #[test]
    fn a_placeholder_index_is_digits_only() {
        let vault = Vault::default();
        vault.store(SecretString::from("hunter2"));
        let signed = format!("{PLACEHOLDER_PREFIX}+0");
        let cmd = guarded(&TOKEN, &vault).mut_arg("token", |arg| arg.default_value(signed));
        assert_eq!(token(cmd, &["app"]), Err(ErrorKind::ValueValidation));
    }

    fn help_of(
        attr: &'static PromptAttr,
        after_help: Option<&'static str>,
        times: usize,
    ) -> String {
        let arg = Arg::new("token").long("token").env("APP_TOKEN").help("API token");
        let cmd = Command::new("app").arg(arg);
        let cmd = match after_help {
            Some(after_help) => cmd.after_help(after_help),
            None => cmd,
        };
        let prompted_args = [PromptedArg { attr, subcommand_path: Vec::new() }];
        let mut cmd = (0..times)
            .fold(cmd, |cmd, _| guard(cmd, &prompted_args, &Vault::default(), "Environment:"));
        cmd.render_help().to_string()
    }

    #[test]
    fn a_refused_secret_is_hidden_and_its_env_var_listed() {
        let help = help_of(&TOKEN, None, 1);
        assert!(!help.contains("--token <token>  API token"), "{help}");
        assert!(help.ends_with("Environment:\n  APP_TOKEN  API token\n"), "{help}");
    }

    #[test]
    fn a_secret_the_command_line_may_pass_keeps_its_flag() {
        let help = help_of(&SCRIPTED_TOKEN, None, 1);
        assert!(help.contains("--token <token>  API token [env: APP_TOKEN]"), "{help}");
        assert!(!help.contains("Environment:"), "{help}");
    }

    #[test]
    fn guarding_twice_lists_the_env_var_once() {
        let help = help_of(&TOKEN, None, 2);
        assert_eq!(help.matches("APP_TOKEN").count(), 1, "{help}");
    }

    #[test]
    fn the_commands_own_after_help_follows_the_env_vars() {
        let help = help_of(&TOKEN, Some("See the docs."), 1);
        assert!(help.ends_with("  APP_TOKEN  API token\n\nSee the docs.\n"), "{help}");
    }
}
