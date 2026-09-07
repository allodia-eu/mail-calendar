//! How an account's own sending identity is written on screen.
//!
//! One rule, in one place, because four clients draw a From field and the interesting case is
//! the empty one: an account with no name sends as a bare address, and a client that filled
//! that gap by hand would put words in the sender's mouth (`docs/sending.md` rule 3).

/// The From label for an account: `Name <address>`, or the address alone when no name is set.
///
/// The same shape the recipient's own client shows, so what the sender picks here and what
/// arrives are read the same way.
///
/// Whitespace-only is empty. A name never survives as a lone pair of angle brackets around an
/// address, which is what a bare `is_empty` check produces the first time somebody's stored
/// name is a space.
#[must_use]
pub fn sender_label(name: &str, email: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return email.to_owned();
    }
    format!("{name} <{email}>")
}

#[cfg(test)]
mod tests {
    use super::sender_label;

    #[test]
    fn a_named_account_reads_as_name_then_address() {
        assert_eq!(
            sender_label("Ada Lovelace", "ada@example.com"),
            "Ada Lovelace <ada@example.com>"
        );
    }

    #[test]
    fn an_account_with_no_name_is_its_address_alone() {
        // Not "<ada@example.com>", and never the address doubled as a name: the app does not
        // invent one (`docs/sending.md` rule 3).
        assert_eq!(sender_label("", "ada@example.com"), "ada@example.com");
        assert_eq!(sender_label("   ", "ada@example.com"), "ada@example.com");
    }

    #[test]
    fn a_name_is_trimmed_rather_than_written_against_the_bracket() {
        assert_eq!(
            sender_label("  Ada  ", "ada@example.com"),
            "Ada <ada@example.com>"
        );
    }

    #[test]
    fn the_name_is_passed_through_rather_than_quoted_or_escaped() {
        // This is a screen label, not a header. The core sanitises what may be stored, and the
        // engine quotes what goes on the wire; a second opinion here would show the user
        // something other than the name they typed.
        assert_eq!(
            sender_label("Ada, Lovelace", "ada@example.com"),
            "Ada, Lovelace <ada@example.com>"
        );
    }
}
