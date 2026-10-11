//! Operating System utilities.

#[cfg(feature = "interactive-editor")]
pub(crate) mod command;
#[cfg(feature = "interactive-editor")]
pub(crate) mod editor;
#[cfg(feature = "interactive-editor")]
pub(crate) mod locale;
pub(crate) mod signal;
#[cfg(feature = "interactive-editor")]
pub(crate) mod term;

#[cfg(feature = "interactive-editor")]
use std::{ffi::OsString, path::Path};

/// The running program's name, like `git`: as it was called, else as its file is named.
#[cfg(feature = "interactive-editor")]
pub(crate) fn process_name() -> Option<OsString> {
    let called = std::env::args_os().next().filter(|called| !called.is_empty());
    let path = called.map(Into::into).or_else(|| std::env::current_exe().ok())?;
    Path::new(&path).file_stem().map(ToOwned::to_owned)
}
