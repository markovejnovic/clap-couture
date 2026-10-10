//! Keeps [`SecretField`](super::SecretField) to the types the prompting parse can read back, so
//! another crate's impl can't compile only to fail when clap hands the value back.

use secrecy::SecretString;

/// The supertrait only this crate can implement.
pub trait Sealed {}

impl Sealed for SecretString {}
