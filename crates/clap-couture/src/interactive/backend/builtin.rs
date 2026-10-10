//! What the built-in backends share.

/// A clap ANSI color, for a backend to convert into its own color type.
pub(super) struct NamedColor(pub(super) clap::builder::styling::AnsiColor);

/// A clap color, for a backend to convert into its own color type.
pub(super) struct ClapColor(pub(super) clap::builder::styling::Color);

/// A clap style, for a backend to convert into its own style type.
pub(super) struct ClapStyle<'style>(pub(super) &'style clap::builder::styling::Style);
