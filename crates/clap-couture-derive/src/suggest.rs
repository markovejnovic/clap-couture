//! "Did you mean" suggestions for a misspelled name.

use core::fmt;

/// The known name closest to a written one, suggested when the written one is unknown.
pub(crate) struct Suggestion<'known>(&'known str);

impl<'known> Suggestion<'known> {
    /// The Jaro-Winkler similarity a known name must exceed to be suggested, as darling's.
    const MIN_SIMILARITY: f64 = 0.8;

    /// The most similar of `known` to `written`, if any is similar enough.
    pub(crate) fn closest(
        written: &str,
        known: impl IntoIterator<Item = &'known str>,
    ) -> Option<Self> {
        known
            .into_iter()
            .map(|known| (strsim::jaro_winkler(written, known), known))
            .filter(|(similarity, _)| *similarity > Self::MIN_SIMILARITY)
            .max_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, known)| Self(known))
    }
}

impl fmt::Display for Suggestion<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "did you mean `{}`?", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Suggestion;

    #[test]
    fn suggests_the_closest_similar_name() {
        let suggestion = Suggestion::closest("depoly", ["account", "deploy", "deplete"]);
        assert_eq!(suggestion.map(|it| it.to_string()).as_deref(), Some("did you mean `deploy`?"));
    }

    #[test]
    fn suggests_nothing_when_no_name_is_similar() {
        assert!(Suggestion::closest("billing", ["account", "deploy"]).is_none());
    }
}
