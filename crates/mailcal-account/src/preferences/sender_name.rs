//! The name an account's outgoing mail is sent under, and the accessors that read and
//! record it.
//!
//! Its own module for the sanitising, which is the part that matters. A display name is
//! free text a user types or pastes, and it is spliced into a `From` header: a carriage
//! return in it is not a formatting problem, it is a second header. So a name is
//! normalised **on the way in**, once, here, rather than checked at every place one is
//! read.
//!
//! Normalising rather than refusing is deliberate. Nobody types a control character; one
//! arrives by pasting a line out of a document, and refusing that paste tells a user their
//! own name is invalid. The engine refuses the same bytes when a draft is assembled, which
//! is the backstop; this is what keeps anyone from reaching it.
//!
//! What the name is *for*, and which accounts can also push it to the server, is
//! `docs/sending.md`.

use super::Preferences;

/// The longest sender name kept, in characters.
///
/// Not a protocol limit: RFC 5322 bounds a header's line length, not a display name, and
/// the engine folds and encodes what it is given. It is a bound on a field a person fills
/// in, long enough that no real name approaches it and short enough that a pasted
/// paragraph never reaches a server.
pub const MAX_SENDER_NAME_CHARS: usize = 128;

/// Normalises a typed or pasted name into one that can safely become a `From` display
/// name: control characters become spaces, runs of whitespace collapse to one, the ends
/// are trimmed, and the result is cut to [`MAX_SENDER_NAME_CHARS`].
///
/// Returns an empty string for a name that is nothing but whitespace, which is how the
/// user clears the field.
#[must_use]
pub fn sanitize_sender_name(input: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in input.chars() {
        // A control character is the header-injection shape, and `char::is_whitespace`
        // does not cover all of them (a NUL is not whitespace), so both are folded to the
        // same single space rather than dropped: dropping would silently join two words.
        if ch.is_control() || ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        if out.chars().count() >= MAX_SENDER_NAME_CHARS {
            break;
        }
        out.push(ch);
    }
    out
}

impl Preferences {
    /// The name `account` sends under, or `None` when nobody has set one.
    ///
    /// `None` and an empty string are the same state and only `None` is representable: an
    /// account whose name is cleared has its entry dropped, so the file never accumulates a
    /// blank row per account the user merely opened.
    #[must_use]
    pub fn sender_name_of(&self, account: &str) -> Option<&str> {
        self.account_sender_names.get(account).map(String::as_str)
    }

    /// Records the name `account` sends under, sanitising it first. A name that is empty
    /// after sanitising clears the entry. Returns whether anything changed, so a client
    /// re-asserting the value it already holds costs no disk write.
    pub fn set_account_sender_name(&mut self, account: &str, name: &str) -> bool {
        let clean = sanitize_sender_name(name);
        if clean.is_empty() {
            return self.account_sender_names.remove(account).is_some();
        }
        if self.sender_name_of(account) == Some(clean.as_str()) {
            return false;
        }
        self.account_sender_names.insert(account.to_owned(), clean);
        true
    }

    /// Forgets `account`'s sender name, on removal, so a re-added id does not inherit a
    /// name the user set for a different mailbox. Returns whether anything was stored.
    pub fn remove_account_sender_name(&mut self, account: &str) -> bool {
        self.account_sender_names.remove(account).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_name_survives_unchanged() {
        assert_eq!(sanitize_sender_name("Dennis Ameling"), "Dennis Ameling");
        // Non-ASCII is ordinary: the engine encodes it as an RFC 2047 word.
        assert_eq!(sanitize_sender_name("Renée Müller"), "Renée Müller");
    }

    #[test]
    fn a_pasted_newline_becomes_a_space_rather_than_a_second_header() {
        // The whole reason this function exists. Dropping the character instead would
        // silently join the two words into one.
        assert_eq!(
            sanitize_sender_name("Alice\r\nBcc: eve@example.com"),
            "Alice Bcc: eve@example.com"
        );
        assert_eq!(sanitize_sender_name("Alice\u{0}Smith"), "Alice Smith");
    }

    #[test]
    fn surrounding_and_repeated_whitespace_is_normalised() {
        assert_eq!(sanitize_sender_name("  Alice   Smith \t"), "Alice Smith");
        assert_eq!(sanitize_sender_name("   "), "");
    }

    #[test]
    fn an_over_long_name_is_cut_to_the_cap_in_characters() {
        // Characters, not bytes: a name of accented letters is not half as long as an
        // ASCII one.
        let long = "é".repeat(MAX_SENDER_NAME_CHARS + 50);
        assert_eq!(
            sanitize_sender_name(&long).chars().count(),
            MAX_SENDER_NAME_CHARS
        );
        let ascii = "a".repeat(MAX_SENDER_NAME_CHARS);
        assert_eq!(sanitize_sender_name(&ascii), ascii);
    }

    #[test]
    fn setting_and_clearing_a_name_round_trips() {
        let mut prefs = Preferences::default();
        assert!(prefs.set_account_sender_name("acct", "  Dennis Ameling "));
        assert_eq!(prefs.sender_name_of("acct"), Some("Dennis Ameling"));
        // Re-asserting the same value writes nothing.
        assert!(!prefs.set_account_sender_name("acct", "Dennis Ameling"));
        // Clearing drops the row rather than storing a blank one.
        assert!(prefs.set_account_sender_name("acct", "   "));
        assert_eq!(prefs.sender_name_of("acct"), None);
        assert!(!prefs.account_sender_names.contains_key("acct"));
    }

    #[test]
    fn removing_an_account_forgets_its_name() {
        let mut prefs = Preferences::default();
        prefs.set_account_sender_name("acct", "Dennis");
        assert!(prefs.remove_account_sender_name("acct"));
        assert!(!prefs.remove_account_sender_name("acct"));
        assert_eq!(prefs.sender_name_of("acct"), None);
    }
}
