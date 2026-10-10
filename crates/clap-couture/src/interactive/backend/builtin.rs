//! What the built-in backends share.

use secrecy::SecretString;
use zeroize::Zeroizing;

#[cfg(feature = "interactive-editor")]
use crate::interactive::PromptError;

/// How an editor prompt ended, for the line a backend leaves in the transcript.
#[cfg(feature = "interactive-editor")]
pub(super) enum Edited {
    /// Answered, shown as the answer's first line, with `…` when more follow; empty for an
    /// optional arg left unset.
    Answered(String),
    /// Backed out of, or answered with nothing where an answer is required.
    Cancelled,
    /// The editor failed; its error is reported once parsing ends.
    Failed,
}

#[cfg(feature = "interactive-editor")]
impl Edited {
    /// How `answer`, the editor's outcome for a prompt that is `optional` or not, ended.
    pub(super) fn of(answer: &Result<Option<String>, PromptError>, optional: bool) -> Self {
        match answer {
            Ok(Some(answer)) => Self::Answered(match answer.split_once('\n') {
                Some((first_line, _)) => format!("{first_line}\u{2026}"),
                None => answer.clone(),
            }),
            Ok(None) if optional => Self::Answered(String::new()),
            Ok(None) | Err(PromptError::Cancelled | PromptError::Signalled(_)) => Self::Cancelled,
            Err(_) => Self::Failed,
        }
    }
}

/// A clap ANSI color, for a backend to convert into its own color type.
pub(super) struct NamedColor(pub(super) clap::builder::styling::AnsiColor);

/// A clap color, for a backend to convert into its own color type.
pub(super) struct ClapColor(pub(super) clap::builder::styling::Color);

/// A clap style, for a backend to convert into its own style type.
pub(super) struct ClapStyle<'style>(pub(super) &'style clap::builder::styling::Style);

/// Move a backend's answer into a `SecretString`, `None` when empty.
///
/// `SecretString::from(String)` shrinks the buffer, which may reallocate and leave the old one
/// behind; copying into an exact fit and zeroizing the original, spare capacity included, doesn't.
pub(super) fn into_secret(answer: String) -> Option<SecretString> {
    let answer = Zeroizing::new(answer);
    (!answer.is_empty()).then(|| SecretString::from(answer.as_str()))
}
