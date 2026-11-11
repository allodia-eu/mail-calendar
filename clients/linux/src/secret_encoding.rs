//! How a value is written into a Secret Service item: as one line.
//!
//! gnome-keyring's plain-text keyring format (the passwordless keyring some distributions set up
//! by default) writes each secret as one `key=value` line without escaping a newline in it. The
//! first line of a multi-line value stays the item's secret and every later line is read back as
//! part of the file, so a stored account config comes back as its first line alone. Encoding the
//! value as base64 keeps it on one line in any keyring format.
//!
//! A value stored before this encoding carries no prefix and is read back as it is.

use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Marks an encoded value. No stored config or index can begin with it: a TOML document cannot
/// open with `base64:` and the index is a JSON array.
const PREFIX: &str = "base64:";

/// `value` as the single line stored in the keyring.
pub(crate) fn encode(value: &[u8]) -> Vec<u8> {
    let mut line = PREFIX.as_bytes().to_vec();
    line.extend_from_slice(STANDARD.encode(value).as_bytes());
    line
}

/// The value a stored line holds: decoded when it is encoded, as stored otherwise.
pub(crate) fn decode(stored: Vec<u8>) -> Result<Vec<u8>, String> {
    match stored.strip_prefix(PREFIX.as_bytes()) {
        Some(encoded) => STANDARD
            .decode(encoded)
            .map_err(|error| format!("a stored value is not valid base64 ({error})")),
        None => Ok(stored),
    }
}

/// An account's config as the core should judge it: decoded, or empty when it cannot be. The core
/// cannot read an empty config, so it leaves out and logs that one account like any other it
/// cannot parse; failing the read instead would lose every other account with it.
pub(crate) fn decode_config(stored: Vec<u8>) -> String {
    decode(stored)
        .ok()
        .and_then(|value| String::from_utf8(value).ok())
        .unwrap_or_default()
}

/// Whether a stored config is one a plain-text keyring cut short before this encoding existed.
///
/// Every config the core writes ends its lines with a newline, so a complete one always holds at
/// least one. The plain-text keyring kept only the first line, without its newline, so an
/// unencoded value with none in it is the remnant of a config whose rest is gone for good. An
/// encoded value never is.
pub(crate) fn is_cut_short(stored: &[u8]) -> bool {
    !stored.starts_with(PREFIX.as_bytes()) && !stored.contains(&b'\n')
}

#[cfg(test)]
mod tests {
    use super::{decode, decode_config, encode, is_cut_short};

    const CONFIG: &str = "capabilities = [\"mail\"]\n\n[google]\nemail = \"a@example.test\"\nrefresh_token = \"t\"\n";

    #[test]
    fn a_multi_line_value_is_stored_as_one_line_and_reads_back_whole() {
        let stored = encode(CONFIG.as_bytes());

        assert!(!stored.contains(&b'\n'), "the stored value spans lines");
        assert!(!stored.contains(&b'\r'), "the stored value spans lines");
        assert_eq!(decode(stored).expect("decodes"), CONFIG.as_bytes());
    }

    #[test]
    fn a_value_stored_before_the_encoding_reads_back_as_it_was() {
        assert_eq!(
            decode(CONFIG.as_bytes().to_vec()).expect("legacy value"),
            CONFIG.as_bytes()
        );
        assert_eq!(
            decode(b"[\"a@example.test@imap.example.test\"]".to_vec()).expect("legacy index"),
            b"[\"a@example.test@imap.example.test\"]"
        );
    }

    #[test]
    fn a_damaged_encoded_value_is_an_error_not_garbage() {
        assert!(decode(b"base64:not base64!".to_vec()).is_err());
    }

    #[test]
    fn a_config_that_cannot_be_decoded_reaches_the_core_empty() {
        assert_eq!(decode_config(encode(CONFIG.as_bytes())), CONFIG);
        assert_eq!(decode_config(b"base64:not base64!".to_vec()), "");
        assert_eq!(decode_config(encode(&[b'a', 0xff])), "");
    }

    #[test]
    fn only_an_unencoded_value_without_a_newline_is_cut_short() {
        let first_line = CONFIG.lines().next().expect("a first line");

        assert!(is_cut_short(first_line.as_bytes()));
        assert!(!is_cut_short(CONFIG.as_bytes()));
        assert!(!is_cut_short(&encode(CONFIG.as_bytes())));
        assert!(!is_cut_short(&encode(first_line.as_bytes())));
    }
}
