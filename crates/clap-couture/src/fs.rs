use alloc::borrow::Cow;
use std::ffi::{OsStr, OsString};

/// `stem` with anything but letters, digits, `-` and `_` replaced, to name a file with.
pub(crate) fn file_safe(stem: &OsStr) -> Cow<'_, OsStr> {
    let is_safe = |char: char| char.is_alphanumeric() || char == '-' || char == '_';
    let text = stem.to_string_lossy();
    // Lossy text that borrowed was valid Unicode, so `stem` itself can be handed back.
    if matches!(text, Cow::Borrowed(_)) && text.chars().all(is_safe) {
        return Cow::Borrowed(stem);
    }
    let safe: String = text.chars().map(|char| if is_safe(char) { char } else { '-' }).collect();
    Cow::Owned(OsString::from(safe))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::separators("my tool/sub-notes_1", "my-tool-sub-notes_1")]
    #[case::extension_dot("deploy.notes", "deploy-notes")]
    #[case::unicode_letters("d\u{e9}ploy", "d\u{e9}ploy")]
    fn file_safe_replaces_what_isnt_safe(#[case] stem: &str, #[case] expected: &str) {
        assert_eq!(file_safe(OsStr::new(stem)), OsStr::new(expected));
    }

    #[test]
    fn a_safe_stem_is_borrowed() {
        assert!(matches!(file_safe(OsStr::new("deploy-notes_1")), Cow::Borrowed(_)));
    }

    #[cfg(unix)]
    #[test]
    fn a_stem_that_isnt_unicode_is_replaced() {
        use std::os::unix::ffi::OsStrExt as _;

        assert_eq!(file_safe(OsStr::from_bytes(b"caf\xE9")), OsStr::new("caf-"));
    }
}
