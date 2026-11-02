//! Which `tel:` URIs this product sends, keeps and opens: RFC 3966's grammar, less `*` and `#`.
//!
//! The RFC allows both in a local number and leaves their meaning undefined. In practice they are
//! service codes, which can forward the reader's calls elsewhere, and Apple's Phone app refuses to
//! dial a link holding either for that reason. They are refused written out and percent-encoded,
//! anywhere in the URI, because a dialler may decode a parameter into the number it shows.

use std::borrow::Cow;

/// `value` less the spaces of a `tel:` number, written as spaces or as `%20`, and unchanged when it
/// is not one. A number is written with spaces between its groups and a `tel:` URI holds none
/// (RFC 3966 §5.1.1), so they are dropped rather than refused.
#[must_use]
pub fn without_number_spaces(value: &str) -> Cow<'_, str> {
    if !value
        .trim_start()
        .get(..4)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("tel:"))
    {
        return Cow::Borrowed(value);
    }
    let spaceless: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    Cow::Owned(spaceless.replace("%20", ""))
}

/// Whether `subscriber`, what follows `tel:`, is a number RFC 3966 can write with no `*` or `#`:
/// a global number (`+` and digits), or a local one with exactly one `phone-context`.
pub(crate) fn is_telephone_subscriber(subscriber: &str) -> bool {
    if subscriber.contains(['*', '#']) {
        return false;
    }
    let mut parts = subscriber.split(';');
    let number = parts.next().unwrap_or_default();
    let mut contexts = 0;
    for par in parts {
        let (name, value) = par
            .split_once('=')
            .map_or((par, None), |(n, v)| (n, Some(v)));
        let valid = if name.eq_ignore_ascii_case("phone-context") {
            contexts += 1;
            value.is_some_and(is_descriptor)
        } else if name.eq_ignore_ascii_case("ext") {
            value.is_some_and(|v| is_number_digits(v, false))
        } else if name.eq_ignore_ascii_case("isub") {
            value.is_some_and(|v| is_escaped(v, is_uric))
        } else {
            is_pname(name) && value.is_none_or(|v| is_escaped(v, is_paramchar))
        };
        if !valid {
            return false;
        }
    }
    match number.strip_prefix('+') {
        Some(digits) => contexts == 0 && is_number_digits(digits, false),
        None => contexts == 1 && is_number_digits(number, true),
    }
}

/// Digits with the visual separators `-`, `.`, `(` and `)`, at least one of them a digit;
/// hexadecimal for a local number.
fn is_number_digits(value: &str, hex: bool) -> bool {
    let digit = |c: char| {
        if hex {
            c.is_ascii_hexdigit()
        } else {
            c.is_ascii_digit()
        }
    };
    value.chars().any(digit)
        && value
            .chars()
            .all(|c| digit(c) || matches!(c, '-' | '.' | '(' | ')'))
}

/// A `phone-context`: a global number's digits or a domain name.
fn is_descriptor(value: &str) -> bool {
    match value.strip_prefix('+') {
        Some(digits) => is_number_digits(digits, false),
        None => is_domain_name(value),
    }
}

/// RFC 3966's `domainname`: dot-separated labels of letters, digits and inner hyphens, the last
/// starting with a letter, with an optional final dot.
fn is_domain_name(value: &str) -> bool {
    let name = value.strip_suffix('.').unwrap_or(value);
    let is_label = |label: &str| {
        label.starts_with(|c: char| c.is_ascii_alphanumeric())
            && label.ends_with(|c: char| c.is_ascii_alphanumeric())
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    name.split('.').all(is_label)
        && name
            .rsplit('.')
            .next()
            .is_some_and(|top| top.starts_with(|c: char| c.is_ascii_alphabetic()))
}

/// `pname`: letters, digits and hyphens.
fn is_pname(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// One or more characters `allowed` admits or `%` escapes, none of which decodes to `*` or `#`.
fn is_escaped(value: &str, allowed: fn(char) -> bool) -> bool {
    let mut chars = value.chars();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() != 2 || !hex.chars().all(|h| h.is_ascii_hexdigit()) {
                return false;
            }
            if matches!(u8::from_str_radix(&hex, 16), Ok(b'*' | b'#')) {
                return false;
            }
        } else if !allowed(c) {
            return false;
        }
    }
    any
}

/// RFC 3966's `unreserved`, less `*`.
fn is_unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '!' | '~' | '\'' | '(' | ')')
}

/// RFC 3966's `paramchar`, less `*` and the escape.
fn is_paramchar(c: char) -> bool {
    is_unreserved(c) || matches!(c, '[' | ']' | '/' | ':' | '&' | '+' | '$')
}

/// RFC 2396's `uric`, less `*` and the escape. `;` never reaches it: it separates parameters.
fn is_uric(c: char) -> bool {
    is_unreserved(c) || matches!(c, '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | ',')
}

#[cfg(test)]
mod tests {
    use super::{is_telephone_subscriber, without_number_spaces};

    #[test]
    fn accepts_the_numbers_rfc_3966_writes() {
        for good in [
            "+15032223333",
            "+44-20-7946-0958;ext=1234",
            "+1(503)222.3333",
            "+1-201-555-0123;isub=1234",
            "+15032223333;foo=bar",
            "7042;phone-context=example.com",
            "863-1234;phone-context=+1-914-555",
            "a1b2;phone-context=example.com.",
            "7042;PHONE-CONTEXT=example.com;EXT=12",
        ] {
            assert!(
                is_telephone_subscriber(good),
                "expected {good:?} to be kept"
            );
        }
    }

    #[test]
    fn refuses_service_codes_written_out_or_encoded() {
        for code in [
            "*21*+31612345678#",
            "*61#;phone-context=example.com",
            "+1503#",
            "+1503;ext=1*2",
            "+1503;x=%2A21%23",
            "+1503;x=%2a",
            "+1503;isub=%23",
        ] {
            assert!(
                !is_telephone_subscriber(code),
                "expected {code:?} to be refused"
            );
        }
    }

    #[test]
    fn refuses_what_the_grammar_does_not_write() {
        for bad in [
            "",
            "+",
            "+-()",
            "+1 503",
            "+1503;",
            "+1503;ext=",
            "+1503;x=%+1",
            "+1503;x=%2",
            "+1503;phone-context=example.com",
            "112",
            "7042;phone-context=example.com;phone-context=example.org",
            "7042;phone-context=-example.com",
            "7042;phone-context=example.1com",
            "15032223333",
        ] {
            assert!(
                !is_telephone_subscriber(bad),
                "expected {bad:?} to be refused"
            );
        }
    }

    #[test]
    fn drops_the_spaces_of_a_number_and_nothing_else() {
        assert_eq!(
            without_number_spaces("tel:+1 503 222 3333"),
            "tel:+15032223333"
        );
        assert_eq!(
            without_number_spaces("TEL:+1%20503\u{a0}222"),
            "TEL:+1503222"
        );
        assert_eq!(without_number_spaces("https://a b/%20"), "https://a b/%20");
    }
}
