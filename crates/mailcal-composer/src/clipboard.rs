//! The HTML a host reads off the clipboard, as text.
//!
//! A clipboard's HTML is bytes with nothing beside them to name their encoding, and the apps that
//! put it there disagree: most write UTF-8, some UTF-16, and older ones whatever their `<meta>`
//! says. Read as UTF-8 regardless, a "·" a Windows-1252 document wrote becomes U+FFFD, and UTF-16
//! becomes nothing the editor can read.

use encoding_rs::{Encoding, REPLACEMENT, UTF_16BE, UTF_16LE, WINDOWS_1252};

/// How far into the markup a declared charset is looked for: the span HTML's own prescan reads.
const PRESCAN_BYTES: usize = 1024;

/// The clipboard's HTML as text, whatever encoding it was written in.
///
/// Decided in this order: a byte-order mark; UTF-16 without one, told by the zero byte beside the
/// markup's first character; markup that is valid UTF-8, which nearly all is; the charset a
/// `<meta>` near the top declares; and Windows-1252, which is what a browser assumes of HTML that
/// says nothing.
#[must_use]
pub fn clipboard_html(bytes: &[u8]) -> String {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        return encoding.decode_with_bom_removal(bytes).0.into_owned();
    }
    if let Some(encoding) = unmarked_utf16(bytes) {
        return encoding.decode_without_bom_handling(bytes).0.into_owned();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    let encoding = declared(bytes).unwrap_or(WINDOWS_1252);
    encoding.decode_without_bom_handling(bytes).0.into_owned()
}

/// UTF-16 with no byte-order mark: markup starts with an ASCII character, so one of its first two
/// bytes is zero.
fn unmarked_utf16(bytes: &[u8]) -> Option<&'static Encoding> {
    match bytes {
        [first, 0, ..] if *first != 0 => Some(UTF_16LE),
        [0, second, ..] if *second != 0 => Some(UTF_16BE),
        _ => None,
    }
}

/// The encoding a `<meta charset="…">` or `<meta content="text/html; charset=…">` names near the
/// top of the markup, if it names one this decoder knows.
fn declared(bytes: &[u8]) -> Option<&'static Encoding> {
    let head = bytes[..bytes.len().min(PRESCAN_BYTES)].to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = find(&head[from..], b"charset") {
        let mut rest = &head[from + at + b"charset".len()..];
        rest = rest.trim_ascii_start();
        if let Some(value) = rest.strip_prefix(b"=") {
            let value = value.trim_ascii_start();
            let value = value
                .strip_prefix(b"\"")
                .or_else(|| value.strip_prefix(b"'"))
                .unwrap_or(value);
            let end = value
                .iter()
                .position(|byte| {
                    matches!(byte, b'"' | b'\'' | b';' | b'>' | b'/') || byte.is_ascii_whitespace()
                })
                .unwrap_or(value.len());
            // A document that says UTF-16 but has no mark and no zero bytes is not UTF-16, which is
            // HTML's own rule: such a label is read as UTF-8. A label that decodes to the
            // replacement encoding would turn the whole paste into one U+FFFD, so it names nothing.
            if let Some(encoding) = Encoding::for_label(&value[..end])
                && encoding != REPLACEMENT
            {
                return Some(encoding.output_encoding());
            }
        }
        from += at + 1;
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::clipboard_html;

    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn utf8_markup_is_read_as_it_is() {
        assert_eq!(
            clipboard_html("<p>· Apples</p>".as_bytes()),
            "<p>· Apples</p>"
        );
    }

    #[test]
    fn utf16_is_read_with_its_byte_order_mark_or_without_one() {
        let mut marked = vec![0xFF, 0xFE];
        marked.extend(utf16le("<p>· Apples</p>"));
        assert_eq!(clipboard_html(&marked), "<p>· Apples</p>");
        assert_eq!(
            clipboard_html(&utf16le("<p>· Apples</p>")),
            "<p>· Apples</p>"
        );
        let big_endian: Vec<u8> = "<p>·</p>"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        assert_eq!(clipboard_html(&big_endian), "<p>·</p>");
    }

    #[test]
    fn a_declared_charset_decides_markup_that_is_not_utf8() {
        let content_type = b"<meta http-equiv=Content-Type content='text/html; charset=windows-1252'><p>\xB7 Apples</p>";
        assert!(clipboard_html(content_type).ends_with("<p>· Apples</p>"));
        let charset = b"<meta charset=\"ISO-8859-1\"><p>caf\xE9</p>";
        assert!(clipboard_html(charset).ends_with("<p>café</p>"));
    }

    #[test]
    fn markup_that_declares_nothing_and_is_not_utf8_is_windows_1252() {
        assert_eq!(
            clipboard_html(b"<p>\x93quoted\x94</p>"),
            "<p>\u{201C}quoted\u{201D}</p>"
        );
    }

    #[test]
    fn a_label_for_the_replacement_encoding_is_not_believed() {
        let bytes = b"<meta charset=iso-2022-kr><p>caf\xE9</p>";
        assert!(clipboard_html(bytes).ends_with("<p>caf\u{E9}</p>"));
    }

    #[test]
    fn a_utf16_label_on_bytes_that_are_not_utf16_is_not_believed() {
        let bytes = b"<meta charset=utf-16><p>caf\xE9</p>";
        assert!(clipboard_html(bytes).ends_with("<p>caf\u{FFFD}</p>"));
    }
}
