//! `#[derive(Couture)]` reads field types and ids exactly as `#[derive(Parser)]` does.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only confirm and text replies are scripted here")]
mod common;

use clap::Parser;
use clap_couture::{Couture, CoutureParser as _};
use common::{Reply, ScriptedBackend};
use rstest::rstest;

/// A field type that reaches the derive through `$t:ty`, wrapped in an invisible group.
macro_rules! cleanup {
    ($flag:ty) => {
        #[derive(Parser, Couture)]
        struct Cleanup {
            #[arg(long)]
            #[couture(confirm = "Delete everything?")]
            force: $flag,
        }
    };
}

/// The arg id clap evaluates, which couture can't read off the attribute.
const REGION_ID: &str = "zone";

cleanup!(bool);

#[derive(Parser, Couture)]
struct Deploy {
    #[arg(long, id = REGION_ID)]
    #[couture(prompt = "Which region?")]
    region: String,
}

#[rstest]
fn a_bool_from_a_macro_is_confirmed() {
    let backend = ScriptedBackend::new([Reply::Confirm(true)]);
    let cleanup = Cleanup::couture_try_parse_from_with(&backend, ["cleanup"]);
    assert_eq!(cleanup.map(|cleanup| cleanup.force).ok(), Some(true));
}

#[rstest]
fn an_arg_id_from_a_const_is_asked() {
    let backend = ScriptedBackend::new([Reply::Text(Some("eu"))]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, ["deploy"]);
    assert_eq!(deploy.map(|deploy| deploy.region).ok().as_deref(), Some("eu"));
}
