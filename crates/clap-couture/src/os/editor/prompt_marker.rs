use std::io;

use anstream::adapter::strip_str;

use crate::strings;

/// What every header line starts with. Markdown headings take `# ` and shell comments `# `, so
/// a user's own line almost never starts with `#|`.
const HEADER: &str = "#|";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptMarker(String);

impl PromptMarker {
    /// Where the header written for `prompt` starts in `lines`: at its last copy of the header's
    /// first line, else, when the user edited that line, at the marked lines ending the file.
    fn header_start(&self, lines: &[&str], prompt: &str) -> usize {
        let prompt = strip_str(prompt).to_string();
        let first = format!("{} {}", self.0, prompt.lines().next().unwrap_or_default());
        lines.iter().rposition(|line| line.trim() == first.trim()).unwrap_or_else(|| {
            let is_answer = |line: &&str| !line.trim().is_empty() && !self.marks(line);
            lines.iter().rposition(is_answer).map_or(0, |last| last.saturating_add(1))
        })
    }

    /// Whether `line` starts with the marker, past any indentation an editor added.
    fn marks(&self, line: &str) -> bool {
        line.trim_start().starts_with(self.0.as_str())
    }

    pub(crate) fn new(marker: impl Into<String>) -> Self {
        Self(marker.into())
    }

    /// The answer in `file`, opened with `prompt`: the header lines and the blank lines around the
    /// rest dropped, each line left ending in a newline as in any text file. `None` when nothing is
    /// left.
    pub(crate) fn strip(&self, file: &str, prompt: Option<&str>) -> Option<String> {
        let is_blank = |line: &&str| line.trim().is_empty();
        let lines: Vec<&str> = file.lines().collect();
        let header_start = prompt.map_or(lines.len(), |prompt| self.header_start(&lines, prompt));
        let (answer, header) = lines.split_at(header_start);
        // Lines typed into the header, or unmarked there, are the user's too.
        let typed_in_header = header.iter().filter(|line| !self.marks(line));
        let mut kept: Vec<&str> =
            answer.iter().chain(typed_in_header).copied().skip_while(is_blank).collect();
        while kept.last().is_some_and(is_blank) {
            kept.pop();
        }
        (!kept.is_empty()).then(|| kept.iter().map(|line| format!("{line}\n")).collect())
    }

    /// The file the editor opens: the answer so far, a blank line, then the header.
    pub(crate) fn write(
        &self,
        out: &mut impl io::Write,
        contents: &str,
        prompt: Option<&str>,
    ) -> io::Result<()> {
        if !contents.is_empty() {
            write!(out, "{contents}")?;
            if !contents.ends_with('\n') {
                writeln!(out)?;
            }
        }
        if let Some(prompt) = prompt {
            writeln!(out)?;
            // The marker, then a space before the text.
            // A styled question's escapes would show in the file as text.
            let prompt = strip_str(prompt).to_string();
            strings::prefix_lines_into(out, &format!("{} ", self.0), [prompt.as_str()])?;
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
    fn render_drops_a_styled_prompts_escapes() {
        let file = rendered("", Some("Release notes, in \u{1b}[1mmarkdown\u{1b}[0m"));
        assert_eq!(file, "\n#| Release notes, in markdown\n");
    }

    #[test]
    fn strip_finds_a_styled_prompts_header() {
        let prompt = "\u{1b}[1mRelease\u{1b}[0m notes";
        let file = format!("#| Release notes\nbody\n{}", rendered("", Some(prompt)));
        assert_eq!(
            PromptMarker::default().strip(&file, Some(prompt)).as_deref(),
            Some("#| Release notes\nbody\n")
        );
    }

    #[test]
    fn render_without_a_prompt_is_the_contents() {
        assert_eq!(rendered("Fixed it.", None), "Fixed it.\n");
    }

    #[test]
    fn strip_round_trips_a_rendered_answer() {
        let answer = "# Heading\n\n  indented: yes  \n\nlast\n";
        assert_eq!(
            PromptMarker::default().strip(&rendered(answer, Some(PROMPT)), Some(PROMPT)).as_deref(),
            Some(answer)
        );
    }

    #[rstest]
    #[case::only_the_header(&rendered("", Some(PROMPT)), None)]
    #[case::whitespace(" \n\t\n", None)]
    #[case::blank_lines_around("\n\nhello\n\n", Some("hello\n"))]
    #[case::users_hash_lines_stay("# not a header\nbody\n", Some("# not a header\nbody\n"))]
    #[case::edited_header_line_goes("body\n#| I changed this\n", Some("body\n"))]
    #[case::markdown_and_shell_comments_stay(
        "## Notes\n#!/bin/sh\n#|x\n",
        Some("## Notes\n#!/bin/sh\n")
    )]
    #[case::users_marker_lines_stay(
        "#|> quoted\nbody\n  #| Release notes",
        Some("#|> quoted\nbody\n")
    )]
    #[case::marker_lines_between_users_lines_stay("a\n#| b\nc\n", Some("a\n#| b\nc\n"))]
    #[case::indented_header_goes("body\n\n  #| Release notes\n\t#|\n", Some("body\n"))]
    #[case::typed_below_the_header_stays(
        &format!("{}More.\n", rendered("Fixed it.", Some(PROMPT))),
        Some("Fixed it.\n\nMore.\n")
    )]
    #[case::unmarked_header_line_stays(
        "Fixed it.\n\n#| Release notes\nerror: kept\n#| Leave the file empty to cancel.\n",
        Some("Fixed it.\n\nerror: kept\n")
    )]
    #[case::quoted_question_above_the_header_stays(
        &format!("#| Release notes\nbody\n{}", rendered("", Some(PROMPT))),
        Some("#| Release notes\nbody\n")
    )]
    fn strip_drops_only_header_and_surrounding_blanks(
        #[case] file: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(PromptMarker::default().strip(file, Some(PROMPT)).as_deref(), expected);
    }

    #[test]
    fn strip_without_a_prompt_keeps_marked_lines() {
        assert_eq!(
            PromptMarker::default().strip("body\n#| mine\n", None).as_deref(),
            Some("body\n#| mine\n")
        );
    }

    #[test]
    fn a_custom_marker_renders_and_strips() {
        let mut file = Vec::new();
        let marker = PromptMarker::new("//");
        let written = marker.write(&mut file, "Fixed it.", Some(PROMPT));
        let file = String::from_utf8(file).unwrap_or_default();
        assert!(written.is_ok(), "writing to a `Vec` can't fail");
        assert_eq!(file, "Fixed it.\n\n// Release notes\n//\n// Leave the file empty to cancel.\n");
        assert_eq!(marker.strip(&file, Some(PROMPT)).as_deref(), Some("Fixed it.\n"));
    }
}
