//! The system's locale.

#[cfg(unix)]
use core::ffi::CStr;

#[cfg(unix)]
use encoding_rs::REPLACEMENT;
use encoding_rs::{DecoderResult, Encoding, UTF_8};

/// The ASCII codesets, as the `C` and `POSIX` locales report them. [`Encoding::for_label`] reads
/// them as windows-1252, which takes any byte, so they name no encoding to fall back to.
#[cfg(unix)]
const ASCII_CODESETS: &[&str] = &["ansi_x3.4-1968", "us-ascii"];

/// Codesets as macOS and the BSDs spell them, where that isn't also a label
/// [`Encoding::for_label`] knows, with the label it knows. glibc's spellings already are.
#[cfg(unix)]
const CODESET_LABELS: &[(&str, &str)] = &[
    ("big5hkscs", "big5-hkscs"),
    ("cp949", "windows-949"),
    ("euccn", "gb2312"),
    ("eucjp", "euc-jp"),
    ("euckr", "euc-kr"),
];

/// Text in no encoding that could be read, decoded as best it could be.
#[derive(Debug)]
pub(crate) struct NotUnicode {
    /// How many U+FFFD in `text` mark bytes that couldn't be read, not text that was there.
    pub(crate) marks: usize,
    pub(crate) text: String,
}

impl NotUnicode {
    /// `bytes` as UTF-8, or as the encoding their byte order mark names, each malformed sequence
    /// marked with a U+FFFD.
    fn read(bytes: &[u8]) -> Self {
        let mut decoder = UTF_8.new_decoder();
        let mut unread = bytes;
        let mut readable = Self { marks: 0, text: String::new() };
        loop {
            readable.text.reserve(
                decoder
                    .max_utf8_buffer_length_without_replacement(unread.len())
                    .unwrap_or_default(),
            );
            let (result, read) =
                decoder.decode_to_string_without_replacement(unread, &mut readable.text, true);
            unread = unread.get(read..).unwrap_or_default();
            match result {
                DecoderResult::InputEmpty => return readable,
                DecoderResult::OutputFull => {}
                DecoderResult::Malformed(..) => {
                    readable.text.push(char::REPLACEMENT_CHARACTER);
                    readable.marks = readable.marks.saturating_add(1);
                }
            }
        }
    }
}

/// Representation of the system's locale.
#[derive(Debug)]
pub(crate) struct Locale {
    /// The ANSI code page, which programs that aren't Unicode use.
    #[cfg(windows)]
    code_page: u32,
    /// The codeset, as the C library names it, like `UTF-8`, `EUC-JP` or `eucJP`; empty for a
    /// locale that names none.
    #[cfg(unix)]
    codeset: String,
}

impl Locale {
    pub(crate) fn decode(bytes: &[u8]) -> Result<String, NotUnicode> {
        let legacy = Self::read().and_then(|locale| locale.encoding());
        decode_with(bytes, legacy).ok_or_else(|| NotUnicode::read(bytes))
    }

    /// The encoding the locale uses for text that isn't Unicode. `None` for a locale that names
    /// none, like `C`.
    pub(crate) fn encoding(&self) -> Option<&'static Encoding> {
        #[cfg(windows)]
        {
            codepage::to_encoding_no_replacement(u16::try_from(self.code_page).ok()?)
        }

        #[cfg(unix)]
        {
            let codeset = self.codeset.to_ascii_lowercase();
            if ASCII_CODESETS.contains(&codeset.as_str()) {
                return None;
            }
            let label = CODESET_LABELS
                .iter()
                .find(|(spelling, _)| *spelling == codeset)
                .map_or(codeset.as_str(), |(_, label)| label);
            Encoding::for_label(label.as_bytes()).filter(|encoding| *encoding != REPLACEMENT)
        }

        #[cfg(not(any(unix, windows)))]
        {
            None
        }
    }

    /// Read the system's locale. `None` for a locale the C library doesn't have.
    #[cfg_attr(
        windows,
        expect(
            clippy::unnecessary_wraps,
            reason = "Windows always has a code page; elsewhere the locale can be missing"
        )
    )]
    pub(crate) fn read() -> Option<Self> {
        #[cfg(windows)]
        {
            // SAFETY: takes nothing, reads a process setting.
            let code_page = unsafe { windows_sys::Win32::Globalization::GetACP() };
            Some(Self { code_page })
        }

        // A locale object of our own, used on this thread only, rather than `setlocale`, which
        // would change the locale for the whole process.
        #[cfg(unix)]
        {
            // SAFETY: an empty name asks for the locale the environment names; a null base
            // starts from nothing.
            let locale = unsafe {
                libc::newlocale(libc::LC_CTYPE_MASK, c"".as_ptr(), core::ptr::null_mut())
            };
            if locale.is_null() {
                return None;
            }

            // SAFETY: `locale` is a live locale object; this changes this thread's locale only.
            let previous = unsafe { libc::uselocale(locale) };
            // SAFETY: `CODESET` is a valid item.
            let codeset = unsafe { libc::nl_langinfo(libc::CODESET) };
            let codeset = (!codeset.is_null()).then(|| {
                // SAFETY: a C string that lives while `locale` does, copied before it is freed.
                unsafe { CStr::from_ptr(codeset) }.to_string_lossy().into_owned()
            });

            // SAFETY: `previous` is the locale this thread had.
            unsafe {
                libc::uselocale(previous);
            }
            // SAFETY: `locale` is no longer in use.
            unsafe {
                libc::freelocale(locale);
            }

            Some(Self { codeset: codeset.unwrap_or_default() })
        }

        #[cfg(not(any(unix, windows)))]
        {
            None
        }
    }
}

/// The text of a saved file: in the encoding its byte order mark names, as notepad's "Unicode"
/// saves UTF-16, or else UTF-8, or else `legacy`, the encoding older editors save in. The mark
/// is dropped. `None` when none of them fits.
fn decode_with(bytes: &[u8], legacy: Option<&'static Encoding>) -> Option<String> {
    if let Some((encoding, mark)) = Encoding::for_bom(bytes) {
        let text = bytes.get(mark..).unwrap_or_default();
        return encoding.decode_without_bom_handling_and_without_replacement(text).map(Into::into);
    }
    [Some(UTF_8), legacy.filter(|legacy| *legacy != UTF_8)]
        .into_iter()
        .flatten()
        .find_map(|encoding| encoding.decode_without_bom_handling_and_without_replacement(bytes))
        .map(Into::into)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[cfg(unix)]
    #[rstest]
    #[case::utf8("UTF-8", Some(encoding_rs::UTF_8))]
    #[case::glibc_latin1("ISO-8859-1", Some(encoding_rs::WINDOWS_1252))]
    #[case::bsd_latin9("ISO8859-15", Some(encoding_rs::ISO_8859_15))]
    #[case::glibc_euc_jp("EUC-JP", Some(encoding_rs::EUC_JP))]
    #[case::bsd_euc_jp("eucJP", Some(encoding_rs::EUC_JP))]
    #[case::bsd_euc_kr("eucKR", Some(encoding_rs::EUC_KR))]
    #[case::bsd_euc_cn("eucCN", Some(encoding_rs::GBK))]
    #[case::bsd_big5_hkscs("Big5HKSCS", Some(encoding_rs::BIG5))]
    #[case::bsd_cp949("CP949", Some(encoding_rs::EUC_KR))]
    #[case::shift_jis("SJIS", Some(encoding_rs::SHIFT_JIS))]
    #[case::koi8_r("KOI8-R", Some(encoding_rs::KOI8_R))]
    #[case::glibc_c("ANSI_X3.4-1968", None)]
    #[case::bsd_c("US-ASCII", None)]
    #[case::none_named("", None)]
    #[case::unknown("ARMSCII-8", None)]
    fn encoding_reads_the_codeset(
        #[case] codeset: &str,
        #[case] expected: Option<&'static Encoding>,
    ) {
        assert_eq!(Locale { codeset: codeset.to_owned() }.encoding(), expected);
    }

    #[cfg(windows)]
    #[rstest]
    #[case::western(1252, Some(encoding_rs::WINDOWS_1252))]
    #[case::japanese(932, Some(encoding_rs::SHIFT_JIS))]
    #[case::utf8(65001, Some(encoding_rs::UTF_8))]
    fn encoding_maps_the_code_page(
        #[case] code_page: u32,
        #[case] expected: Option<&'static Encoding>,
    ) {
        assert_eq!(Locale { code_page }.encoding(), expected);
    }

    #[rstest]
    #[case::utf8("h\u{e9}llo".as_bytes(), None, Some("h\u{e9}llo"))]
    #[case::utf8_mark_dropped(b"\xEF\xBB\xBFhi", None, Some("hi"))]
    #[case::utf16_le(b"\xFF\xFEh\x00\xE9\x00", None, Some("h\u{e9}"))]
    #[case::utf16_be(b"\xFE\xFF\x00h\x00\xE9", None, Some("h\u{e9}"))]
    #[case::utf16_odd_byte(b"\xFF\xFEh\x00\xE9", None, None)]
    #[case::utf16_lone_surrogate(b"\xFF\xFE\x00\xD8", None, None)]
    #[case::latin1_without_legacy(b"caf\xE9", None, None)]
    #[case::latin1_as_legacy(b"caf\xE9", Some(encoding_rs::WINDOWS_1252), Some("caf\u{e9}"))]
    #[case::utf8_before_legacy("h\u{e9}".as_bytes(), Some(encoding_rs::WINDOWS_1252), Some("h\u{e9}"))]
    #[case::legacy_utf8_adds_nothing(b"caf\xE9", Some(UTF_8), None)]
    #[case::mark_beats_legacy(b"\xFF\xFEh\x00", Some(encoding_rs::WINDOWS_1252), Some("h"))]
    fn decode_tries_the_mark_then_utf8_then_legacy(
        #[case] bytes: &[u8],
        #[case] legacy: Option<&'static Encoding>,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(decode_with(bytes, legacy).as_deref(), expected);
    }

    #[test]
    fn decode_keeps_what_it_could_read() {
        let decoded = Locale::decode(b"\xFF\xFEh\x00\x00\xD8");
        assert!(matches!(decoded, Err(NotUnicode { marks: _, text }) if text.starts_with('h')));
    }

    #[rstest]
    #[case::latin1(b"caf\xE9", "caf\u{FFFD}", 1)]
    #[case::own_mark_kept(b"\xEF\xBF\xBD caf\xE9", "\u{FFFD} caf\u{FFFD}", 1)]
    #[case::utf16_lone_surrogate(b"\xFF\xFEh\x00\x00\xD8", "h\u{FFFD}", 1)]
    fn not_unicode_counts_only_its_own_marks(
        #[case] bytes: &[u8],
        #[case] text: &str,
        #[case] marks: usize,
    ) {
        let decoded = NotUnicode::read(bytes);
        assert_eq!((decoded.text.as_str(), decoded.marks), (text, marks));
    }
}
