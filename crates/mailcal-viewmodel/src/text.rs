//! Sanitising attacker-controlled text for display.

/// Sanitises an attacker-controlled iCalendar text value for display, returning
/// `(text, was_truncated)`.
///
/// Three things happen, and each one is a defect that was observed rather than imagined:
///
/// 1. **Control characters are dropped** (tabs and newlines become spaces). A bare `\r` or a C1
///    control in a `SUMMARY` corrupts a single-line label, and the bidi overrides (U+202E and
///    friends) let a sender make text read in an order that is not the order it is stored in; the
///    same class of trick the attachment-name sanitiser already blocks.
/// 2. **Whitespace collapses.** iCalendar folds long values across lines; an unfolded `DESCRIPTION`
///    is full of runs of spaces.
/// 3. **It is truncated on a character boundary**, never a byte one, so a multi-byte character is
///    never cut in half.
///
/// It does **not** escape markup, because the contract is that this is *text*: a client renders
/// it as text (`use_markup(false)` on GTK). Escaping here would show a literal `&amp;` to every
/// client that does the right thing.
#[must_use]
pub fn plain_text(value: &str, limit: usize) -> (String, bool) {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_control() || is_bidi_control(ch) {
                ' '
            } else {
                ch
            }
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        return (collapsed, false);
    }
    let truncated: String = collapsed.chars().take(limit).collect();
    (truncated, true)
}

/// The Unicode format controls that let a sender disguise the reading order of text: the same
/// set the engine's attachment-name sanitiser refuses.
fn is_bidi_control(ch: char) -> bool {
    matches!(ch,
        '\u{00AD}'                // soft hyphen
        | '\u{061C}'              // Arabic letter mark
        | '\u{200B}'..='\u{200F}' // zero-width space/joiners, LRM, RLM
        | '\u{202A}'..='\u{202E}' // bidi embeddings and overrides
        | '\u{2066}'..='\u{2069}' // bidi isolates
        | '\u{FEFF}'              // zero-width no-break space / BOM
    )
}

#[cfg(test)]
mod tests {
    use super::plain_text;

    #[test]
    fn attacker_controlled_text_is_stripped_collapsed_and_truncated() {
        // Control characters and the bidi overrides are dropped: a bare CR corrupts a single-line
        // label, and U+202E lets a sender make text read in an order it is not stored in.
        let (text, truncated) = plain_text("Sprint\r\nplanning\u{202E}gniteem\t\tnow", 200);
        assert_eq!(text, "Sprint planning gniteem now");
        assert!(!truncated);

        // Truncation is on a character boundary, never a byte one, so a multi-byte character is
        // never cut in half.
        let (cut, was_cut) = plain_text(&"é".repeat(50), 10);
        assert!(was_cut);
        assert_eq!(cut.chars().count(), 10);
        assert_eq!(cut, "é".repeat(10));
    }

    #[test]
    fn markup_is_left_alone_because_the_contract_is_that_this_is_text() {
        // Escaping here would show a literal `&amp;` on every client that correctly renders these
        // fields as text. The rule is that the *client* must not parse them as markup; on GTK,
        // `use_markup(false)`.
        let (text, _) = plain_text("Q&A <b>bold</b> & more", 200);
        assert_eq!(text, "Q&A <b>bold</b> & more");
    }
}
