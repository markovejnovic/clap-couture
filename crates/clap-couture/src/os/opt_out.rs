//! The environment variables that turn prompts off.

use std::ffi::OsStr;

/// A variable the user set to turn prompts off, as on a pseudo-terminal nobody answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptOptOut {
    /// `CI`, set to anything but `0` or `false`.
    Ci,
    /// `NO_PROMPT`, set to anything.
    NoPrompt,
}

impl PromptOptOut {
    /// The opt-out that `no_prompt` and `ci`, as `NO_PROMPT` and `CI` hold them, make; `None`
    /// when neither does. An empty value counts as unset.
    fn parse(no_prompt: Option<&OsStr>, ci: Option<&OsStr>) -> Option<Self> {
        if no_prompt.is_some_and(|value| !value.is_empty()) {
            return Some(Self::NoPrompt);
        }
        ci.filter(|value| !["", "0", "false"].map(OsStr::new).contains(value)).map(|_| Self::Ci)
    }

    /// Read `$NO_PROMPT` and `$CI`; `None` when neither turns prompts off.
    pub(crate) fn read() -> Option<Self> {
        let no_prompt = std::env::var_os("NO_PROMPT");
        let ci = std::env::var_os("CI");
        Self::parse(no_prompt.as_deref(), ci.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::unset(None, None, None)]
    #[case::no_prompt(Some("1"), None, Some(PromptOptOut::NoPrompt))]
    #[case::no_prompt_any_value(Some("0"), None, Some(PromptOptOut::NoPrompt))]
    #[case::no_prompt_empty_is_unset(Some(""), None, None)]
    #[case::ci_true(None, Some("true"), Some(PromptOptOut::Ci))]
    #[case::ci_one(None, Some("1"), Some(PromptOptOut::Ci))]
    #[case::ci_empty_is_unset(None, Some(""), None)]
    #[case::ci_zero(None, Some("0"), None)]
    #[case::ci_false(None, Some("false"), None)]
    #[case::no_prompt_beats_ci_false(Some("1"), Some("false"), Some(PromptOptOut::NoPrompt))]
    fn parse_reads_the_values(
        #[case] no_prompt: Option<&str>,
        #[case] ci: Option<&str>,
        #[case] expected: Option<PromptOptOut>,
    ) {
        assert_eq!(PromptOptOut::parse(no_prompt.map(OsStr::new), ci.map(OsStr::new)), expected);
    }
}
