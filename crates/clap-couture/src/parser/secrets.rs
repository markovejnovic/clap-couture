//! Secret args: clap only ever sees a placeholder for a prompted answer, and never a value from
//! the command line unless the arg allows it.

use alloc::sync::Arc;
use std::{
    ffi::OsStr,
    sync::{Mutex, MutexGuard, PoisonError},
};

use clap::{Arg, Command, builder::TypedValueParser, error::ErrorKind, parser::ValueSource};
use secrecy::SecretString;

use super::prompting::with_arg_at;
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
        let index: usize = value.strip_prefix(PLACEHOLDER_PREFIX)?.parse().ok()?;
        self.secrets().get(index).cloned()
    }

    // Nothing panics while holding the lock, so a poisoned one still holds whole answers.
    fn secrets(&self) -> MutexGuard<'_, Vec<SecretString>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keep `secret`, returning the placeholder clap gets in its place.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "only the tests store answers until the prompting parse asks for secrets"
        )
    )]
    pub(crate) fn store(&self, secret: SecretString) -> String {
        let mut secrets = self.secrets();
        let placeholder = format!("{PLACEHOLDER_PREFIX}{}", secrets.len());
        secrets.push(secret);
        placeholder
    }
}

/// A secret arg's value parser: placeholders resolve from the vault, the command line is refused
/// unless allowed, and anything else (an env var) is taken as is.
#[derive(Clone)]
struct SecretValueParser {
    /// `secret(allow_argv)`.
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
            return Err(on_command_line(cmd, arg));
        }
        let value =
            value.to_str().ok_or_else(|| clap::Error::new(ErrorKind::InvalidUtf8).with_cmd(cmd))?;
        Ok(self.vault.get(value).unwrap_or_else(|| SecretString::from(value)))
    }
}

/// Give each secret arg in `prompted_args` a [`SecretValueParser`] on `vault`, and keep its env
/// var's value out of `--help`.
pub(crate) fn guard(cmd: Command, prompted_args: &[PromptedArg], vault: &Vault) -> Command {
    prompted_args.iter().fold(cmd, |cmd, prompted_arg| {
        let Some(secret) = &prompted_arg.attr.secret else { return cmd };
        let parser = SecretValueParser { allow_argv: secret.allow_argv, vault: vault.clone() };
        with_arg_at(cmd, &prompted_arg.subcommand_path, prompted_arg.attr.arg_id, |arg| {
            arg.value_parser(parser).hide_env_values(true)
        })
    })
}

/// The refusal of a secret passed on the command line, pointing at the ways it may come in.
fn on_command_line(cmd: &Command, arg: Option<&Arg>) -> clap::Error {
    let name = arg.map_or_else(|| "a secret".to_owned(), ToString::to_string);
    let env = arg.and_then(Arg::get_env).map(OsStr::to_string_lossy);
    let hint = env.map_or_else(
        || "leave it out to be asked".to_owned(),
        |env| format!("set {env} or leave it out to be asked"),
    );
    clap::Error::raw(
        ErrorKind::ArgumentConflict,
        format!("{name} is a secret and can't be passed on the command line; {hint}\n"),
    )
    .with_cmd(cmd)
}

#[cfg(test)]
mod tests {
    use clap::{Arg, Command, error::ErrorKind};
    use secrecy::{ExposeSecret, SecretString};

    use super::{Vault, guard};
    use crate::interactive::{PromptAttr, PromptPurpose, PromptedArg, SecretAttr};

    static TOKEN: PromptAttr = PromptAttr {
        arg_id: "token",
        editor: None,
        purpose: PromptPurpose::Value,
        question: None,
        secret: Some(SecretAttr { allow_argv: false }),
    };

    static SCRIPTED_TOKEN: PromptAttr = PromptAttr {
        arg_id: "token",
        editor: None,
        purpose: PromptPurpose::Value,
        question: None,
        secret: Some(SecretAttr { allow_argv: true }),
    };

    fn guarded(attr: &'static PromptAttr, vault: &Vault) -> Command {
        let cmd = Command::new("app").arg(Arg::new("token").long("token"));
        guard(cmd, &[PromptedArg { attr, subcommand_path: Vec::new() }], vault)
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
    fn the_command_line_is_refused() {
        let cmd = guarded(&TOKEN, &Vault::default());
        assert_eq!(token(cmd, &["app", "--token", "x"]), Err(ErrorKind::ArgumentConflict));
    }

    #[test]
    fn allow_argv_takes_the_command_line() {
        let cmd = guarded(&SCRIPTED_TOKEN, &Vault::default());
        assert_eq!(token(cmd, &["app", "--token", "x"]).as_deref(), Ok("x"));
    }

    #[test]
    fn a_typed_placeholder_lookalike_is_still_refused() {
        let cmd = guarded(&TOKEN, &Vault::default());
        let lookalike = "\0couture-secret:0";
        assert_eq!(token(cmd, &["app", "--token", lookalike]), Err(ErrorKind::ArgumentConflict));
    }
}
