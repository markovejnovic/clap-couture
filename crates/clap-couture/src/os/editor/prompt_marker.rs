use std::io;

use crate::strings;

/// What every header line starts with. Markdown headings take `# ` and shell comments `# `, so
/// a user's own line almost never starts with `#|`.
const HEADER: &str = "#|";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptMarker(String);

impl PromptMarker {
    pub(crate) fn new(marker: impl Into<String>) -> Self {
        Self(marker.into())
    }

    /// The answer in `file`: the header lines and the blank lines around the rest dropped. `None`
    /// when nothing is left.
    pub(crate) fn strip(&self, file: &str) -> Option<String> {
        let is_blank = |line: &&str| line.trim().is_empty();
        let mut kept: Vec<&str> = file
            .lines()
            .filter(|line| !line.starts_with(self.0.as_str()))
            .skip_while(is_blank)
            .collect();
        while kept.last().is_some_and(is_blank) {
            kept.pop();
        }
        (!kept.is_empty()).then(|| kept.join("\n"))
    }

    /// The file the editor opens: the answer so far, a blank line, then the header.
    pub(crate) fn write(
        &self,
        out: &mut impl io::Write,
        contents: &str,
        prompt: Option<&str>,
    ) -> io::Result<()> {
        if !contents.is_empty() {
            writeln!(out, "{contents}")?;
        }
        if let Some(prompt) = prompt {
            writeln!(out)?;
            // The marker, then a space before the text.
            strings::prefix_lines_into(out, &format!("{} ", self.0), [prompt])?;
        }
        Ok(())
    }
}

impl Default for PromptMarker {
    fn default() -> Self {
        Self::new(HEADER)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const PROMPT: &str = "Release notes\n\nLeave the file empty to cancel.";

    fn rendered(contents: &str, prompt: Option<&str>) -> String {
        let mut file = Vec::new();
        let written = PromptMarker::default().write(&mut file, contents, prompt);
        assert!(written.is_ok(), "writing to a `Vec` can't fail");
        String::from_utf8(file).unwrap_or_default()
    }

    #[test]
    fn render_puts_the_answer_before_the_prompt() {
        let file = rendered("Fixed it.", Some(PROMPT));
        assert_eq!(file, "Fixed it.\n\n#| Release notes\n#|\n#| Leave the file empty to cancel.\n");
    }

    #[test]
    fn render_without_a_prompt_is_the_contents() {
        assert_eq!(rendered("Fixed it.", None), "Fixed it.\n");
    }

    #[test]
    fn strip_round_trips_a_rendered_answer() {
        let answer = "# Heading\n\n  indented: yes  \n\nlast";
        assert_eq!(
            PromptMarker::default().strip(&rendered(answer, Some(PROMPT))).as_deref(),
            Some(answer)
        );
    }

    #[rstest]
    #[case::only_the_header(&rendered("", Some(PROMPT)), None)]
    #[case::whitespace(" \n\t\n", None)]
    #[case::blank_lines_around("\n\nhello\n\n", Some("hello"))]
    #[case::users_hash_lines_stay("# not a header\nbody\n", Some("# not a header\nbody"))]
    #[case::edited_header_line_goes("body\n#| I changed this\n", Some("body"))]
    #[case::markdown_and_shell_comments_stay(
        "## Notes\n#!/bin/sh\n#|x\n",
        Some("## Notes\n#!/bin/sh")
    )]
    fn strip_drops_only_header_and_surrounding_blanks(
        #[case] file: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(PromptMarker::default().strip(file).as_deref(), expected);
    }

    #[test]
    fn a_custom_marker_renders_and_strips() {
        let mut file = Vec::new();
        let marker = PromptMarker::new("//");
        let written = marker.write(&mut file, "Fixed it.", Some(PROMPT));
        let file = String::from_utf8(file).unwrap_or_default();
        assert!(written.is_ok(), "writing to a `Vec` can't fail");
        assert_eq!(file, "Fixed it.\n\n// Release notes\n//\n// Leave the file empty to cancel.\n");
        assert_eq!(marker.strip(&file).as_deref(), Some("Fixed it."));
    }
}
