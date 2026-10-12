//! Operating System utilities.

#[cfg(feature = "interactive-editor")]
pub(crate) mod command;
#[cfg(feature = "interactive-editor")]
pub(crate) mod editor;
#[cfg(feature = "interactive-editor")]
pub(crate) mod locale;
pub(crate) mod opt_out;
pub(crate) mod signal;
pub(crate) mod stdin;
#[cfg(feature = "interactive-editor")]
pub(crate) mod term;

#[cfg(feature = "interactive-editor")]
use std::ffi::OsString;
use std::{ffi::OsStr, path::Path};

/// The name of a program called as `called`, like `git` for `/usr/bin/git`, as clap names it.
pub(crate) fn program_name(called: &OsStr) -> Option<&OsStr> {
    Path::new(called).file_name()
}

/// The running program's name, like `git`: as it was called, else as its file is named.
#[cfg(feature = "interactive-editor")]
pub(crate) fn process_name() -> Option<OsString> {
    let called = std::env::args_os().next().filter(|called| !called.is_empty());
    let path = called.map(Into::into).or_else(|| std::env::current_exe().ok())?;
    let name = program_name(path.as_os_str())?;
    Path::new(name).file_stem().map(ToOwned::to_owned)
}
