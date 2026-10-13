//! couture asks for a prompted arg exactly when clap reports it got no value or only its default,
//! and otherwise keeps what clap parsed. Stamped over env vars set, set empty and unset, with and
//! without a default, one value or many, passed or not. The env vars are process-wide, so this
//! binary holds a single test.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only text replies are scripted here")]
mod common;

use clap::{CommandFactory, FromArgMatches, Parser, parser::ValueSource};
use clap_couture::{Couture, CoutureParser};
use common::{Reply, ScriptedBackend};

/// A parser of one prompted `--value`, read from `$env`, with `$default` if any, holding `$ty`.
macro_rules! sourced {
    ($name:ident, $env:literal, [$($default:literal)?], $ty:ty) => {
        #[derive(Parser, Couture, Debug)]
        struct $name {
            #[arg(long, env = $env, value_delimiter = ',' $(, default_value = $default)?)]
            #[couture(prompt = "Value?")]
            value: $ty,
        }

        impl Sourced for $name {
            fn values(&self) -> Vec<String> {
                Holds::values(&self.value)
            }
        }
    };
}

/// A field's values, however the field holds them.
trait Holds {
    fn values(&self) -> Vec<String>;
}

/// A generated parser.
trait Sourced: CoutureParser + CommandFactory + FromArgMatches {
    fn values(&self) -> Vec<String>;
}

impl Holds for Option<String> {
    fn values(&self) -> Vec<String> {
        self.iter().cloned().collect()
    }
}

impl Holds for String {
    fn values(&self) -> Vec<String> {
        Vec::from([self.clone()])
    }
}

impl Holds for Vec<String> {
    fn values(&self) -> Vec<String> {
        self.clone()
    }
}

sourced!(SetOne, "CLAP_COUTURE_SOURCES_SET_ONE", [], Option<String>);
sourced!(SetOneDefault, "CLAP_COUTURE_SOURCES_SET_ONE_DEFAULT", ["dflt"], String);
sourced!(SetMany, "CLAP_COUTURE_SOURCES_SET_MANY", [], Vec<String>);
sourced!(SetManyDefault, "CLAP_COUTURE_SOURCES_SET_MANY_DEFAULT", ["d1,d2"], Vec<String>);
sourced!(EmptyOne, "CLAP_COUTURE_SOURCES_EMPTY_ONE", [], Option<String>);
sourced!(EmptyOneDefault, "CLAP_COUTURE_SOURCES_EMPTY_ONE_DEFAULT", ["dflt"], String);
sourced!(EmptyMany, "CLAP_COUTURE_SOURCES_EMPTY_MANY", [], Vec<String>);
sourced!(EmptyManyDefault, "CLAP_COUTURE_SOURCES_EMPTY_MANY_DEFAULT", ["d1,d2"], Vec<String>);
sourced!(UnsetOne, "CLAP_COUTURE_SOURCES_UNSET_ONE", [], Option<String>);
sourced!(UnsetOneDefault, "CLAP_COUTURE_SOURCES_UNSET_ONE_DEFAULT", ["dflt"], String);
sourced!(UnsetMany, "CLAP_COUTURE_SOURCES_UNSET_MANY", [], Vec<String>);
sourced!(UnsetManyDefault, "CLAP_COUTURE_SOURCES_UNSET_MANY_DEFAULT", ["d1,d2"], Vec<String>);

/// Check that `T` asks for `--value` exactly when clap reports no value or only its default, for
/// each argv; the count checked.
fn check<T>(label: &str) -> usize
where
    T: Sourced,
{
    let argvs: [&[&str]; 3] = [&["app"], &["app", "--value", "typed"], &["app", "--value=a,b"]];
    for argv in argvs {
        let matches = T::command().try_get_matches_from(argv);
        let Ok(mut matches) = matches else { continue };
        let source = matches.value_source("value");
        let asks = matches!(source, None | Some(ValueSource::DefaultValue));
        let clap = T::from_arg_matches_mut(&mut matches).map(|parsed| parsed.values());
        // Enough answers for any prompt: one value, then the empty one ending a list.
        let backend = ScriptedBackend::new([Reply::Text(Some("asked")), Reply::Text(None)]);
        let couture = T::couture_try_parse_from_with(&backend, argv).map(|parsed| parsed.values());
        let asked = !backend.asked().is_empty();
        assert_eq!(asked, asks, "{label} on {argv:?}, which clap sourced from {source:?}");
        let expected = if asks { Ok(Vec::from(["asked".to_owned()])) } else { clap };
        assert_eq!(couture.ok(), expected.ok(), "{label} on {argv:?}");
    }
    argvs.len()
}

#[test]
fn couture_asks_where_clap_sourced_no_value_but_a_default() {
    let set =
        ["SET_ONE", "SET_ONE_DEFAULT", "SET_MANY", "SET_MANY_DEFAULT"].map(|name| (name, "e1,e2"));
    let empty = ["EMPTY_ONE", "EMPTY_ONE_DEFAULT", "EMPTY_MANY", "EMPTY_MANY_DEFAULT"]
        .map(|name| (name, ""));
    for (name, value) in set.into_iter().chain(empty) {
        // SAFETY: this binary holds a single test, so no other thread reads the environment.
        unsafe {
            std::env::set_var(format!("CLAP_COUTURE_SOURCES_{name}"), value);
        }
    }
    let checked = [
        check::<SetOne>("SetOne"),
        check::<SetOneDefault>("SetOneDefault"),
        check::<SetMany>("SetMany"),
        check::<SetManyDefault>("SetManyDefault"),
        check::<EmptyOne>("EmptyOne"),
        check::<EmptyOneDefault>("EmptyOneDefault"),
        check::<EmptyMany>("EmptyMany"),
        check::<EmptyManyDefault>("EmptyManyDefault"),
        check::<UnsetOne>("UnsetOne"),
        check::<UnsetOneDefault>("UnsetOneDefault"),
        check::<UnsetMany>("UnsetMany"),
        check::<UnsetManyDefault>("UnsetManyDefault"),
    ];
    assert_eq!(checked.iter().sum::<usize>(), 36);
}
