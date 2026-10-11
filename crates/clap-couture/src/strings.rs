use std::io;

pub(crate) fn prefix_lines_into<'text>(
    out: &mut impl io::Write,
    prefix: &str,
    texts: impl IntoIterator<Item = &'text str>,
) -> io::Result<()> {
    // No trailing whitespace after the prefix of an empty line.
    let bare_prefix = prefix.trim_end();
    for text in texts {
        // `"".lines()` yields nothing, but an empty text still takes a line.
        if text.is_empty() {
            writeln!(out, "{bare_prefix}")?;
        }
        for line in text.lines() {
            if line.is_empty() {
                writeln!(out, "{bare_prefix}")?;
            } else {
                writeln!(out, "{prefix}{line}")?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn prefixed(prefix: &str, texts: &[&str]) -> String {
        let mut out = Vec::new();
        let written = prefix_lines_into(&mut out, prefix, texts.iter().copied());
        assert!(written.is_ok(), "writing to a `Vec` can't fail");
        String::from_utf8(out).unwrap_or_default()
    }

    #[rstest]
    #[case::one_line("#| ", &["Release notes"], "#| Release notes\n")]
    #[case::several_texts("#| ", &["a", "b"], "#| a\n#| b\n")]
    #[case::multi_line_text("#| ", &["one\ntwo"], "#| one\n#| two\n")]
    #[case::crlf("#| ", &["one\r\ntwo"], "#| one\n#| two\n")]
    #[case::empty_text_is_a_bare_prefix("#| ", &["a", "", "b"], "#| a\n#|\n#| b\n")]
    #[case::empty_line_inside_a_text("#| ", &["a\n\nb"], "#| a\n#|\n#| b\n")]
    #[case::other_prefix("#| error: ", &["bad"], "#| error: bad\n")]
    #[case::nothing("#| ", &[], "")]
    fn prefixes_every_line(#[case] prefix: &str, #[case] texts: &[&str], #[case] expected: &str) {
        assert_eq!(prefixed(prefix, texts), expected);
    }

    #[test]
    fn a_failed_write_is_reported() {
        let mut full: &mut [u8] = &mut [];
        let written = prefix_lines_into(&mut full, "#| ", ["a"]);
        assert_eq!(written.err().map(|err| err.kind()), Some(io::ErrorKind::WriteZero));
    }
}
