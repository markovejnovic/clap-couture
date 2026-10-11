//! What the operating system and the terminal do differently from platform to platform, kept
//! apart from what the crate does with it.

#[cfg(feature = "interactive-editor")]
pub(crate) mod term;
