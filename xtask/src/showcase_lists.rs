//! Reading the locale, screen and appearance lists back out of the capture scripts.
//!
//! Each of these lists is kept twice, once in the shell capture driver and once in the PowerShell
//! one, and the whole point of the checks that use them is that the two copies drift. So every
//! reader here returns what it actually parsed, and an empty result is a real answer the caller
//! must treat as a failure: a parse that finds nothing would otherwise compare "" to "" and pass,
//! which is the failure mode these checks exist to prevent.

/// Words in a `ValidateSet(...)` on a line that also mentions `parameter`.
///
/// The PowerShell drivers write the set and the parameter it constrains on one line, so the
/// parameter name is what tells `-Locale`'s set from `-Screen`'s.
pub(crate) fn validate_set(haystack: &str, parameter: &str) -> Vec<String> {
    for line in haystack.lines() {
        if !line.contains(parameter) {
            continue;
        }
        let Some((_, rest)) = line.split_once("ValidateSet(") else {
            continue;
        };
        let Some((inner, _)) = rest.split_once(')') else {
            continue;
        };
        return crate::git::sorted_words(inner);
    }
    Vec::new()
}

/// The locale codes a `case` arm labels, read out of a shell function's body.
///
/// The arms are lines that are nothing but lowercase letters, spaces and pipes before the closing
/// parenthesis, `en|nl)`, which is what distinguishes them from the commands inside.
pub(crate) fn case_arm_labels(body: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(label) = trimmed.strip_suffix(')') else {
            continue;
        };
        if !label.is_empty()
            && label
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '|')
        {
            words.extend(crate::git::sorted_words(label));
        }
    }
    words.sort();
    words.dedup();
    words
}

/// The words a `linux)` arm of `store_screens_for` prints.
pub(crate) fn linux_store_screens(showcase_sh: &str) -> Vec<String> {
    let body = crate::git::function_body(showcase_sh, "store_screens_for");
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("linux)") else {
            continue;
        };
        // In `linux) printf '%s\n' a b c ;;` the command and its format string are dropped before
        // the words are read. Without that, `printf` itself is shaped exactly like a screen name
        // and is reported as one the client cannot reach.
        let words = rest.trim().strip_suffix(";;").unwrap_or(rest);
        let tokens: Vec<&str> = words.split_whitespace().collect();
        let names = if tokens.first() == Some(&"printf") {
            tokens.get(2..).unwrap_or_default()
        } else {
            &tokens[..]
        };
        return keep(&crate::git::sorted_words(&names.join(" ")), is_screen_name);
    }
    Vec::new()
}

/// The screen names `parse_screen` accepts.
///
/// Read off the arms that return `Ok`, not searched for anywhere in the file. Searching the file is
/// the version of this check that cannot fail: the client's own unit test asserts that a particular
/// name is *refused*, so the literal is present, and a whole-file search happily reports that the
/// client can reach a screen it explicitly rejects.
pub(crate) fn accepted_screens(showcase_rs: &str) -> Vec<String> {
    let body = crate::git::function_body(showcase_rs, "fn parse_screen");
    let body = if body.is_empty() { showcase_rs } else { body };
    let mut words: Vec<String> = Vec::new();
    for line in body.lines() {
        if !line.contains("Ok(ShowcaseScreen::") {
            continue;
        }
        words.extend(quoted_words(line));
    }
    words.sort();
    words.dedup();
    keep(&words, is_screen_name)
}

/// The appearance words `appearances_for` can print.
///
/// Comments are dropped: a comment naming a theme the function does not shoot would make the
/// containment check stricter, but it would also make this list a lie. The words are separated by a
/// literal `\n` inside a `printf`, so those two characters are replaced before the words are read;
/// dropping only the backslash leaves `ndark`, which matches nothing and silently takes `dark` out
/// of the list.
pub(crate) fn appearances(showcase_sh: &str) -> Vec<String> {
    let body = crate::git::function_body(showcase_sh, "appearances_for");
    let mut words: Vec<String> = Vec::new();
    for line in body.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        for word in crate::git::sorted_words(&line.replace("\\n", " ")) {
            let cleaned: String = word.chars().filter(char::is_ascii_lowercase).collect();
            if matches!(cleaned.as_str(), "system" | "light" | "dark") {
                words.push(cleaned);
            }
        }
    }
    words.sort();
    words.dedup();
    words
}

/// The double-quoted words on a line.
fn quoted_words(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some((_, after)) = rest.split_once('"') {
        let Some((word, tail)) = after.split_once('"') else {
            break;
        };
        found.push(word.to_owned());
        rest = tail;
    }
    found
}

/// Keeps only the words satisfying `shape`.
pub(crate) fn keep(words: &[String], shape: fn(&str) -> bool) -> Vec<String> {
    words.iter().filter(|w| shape(w)).cloned().collect()
}

/// A two-letter locale code.
pub(crate) fn is_locale(word: &str) -> bool {
    word.len() == 2 && word.chars().all(|c| c.is_ascii_lowercase())
}

/// A screen name: lowercase, at least two characters, hyphens allowed after the first.
pub(crate) fn is_screen_name(word: &str) -> bool {
    word.len() >= 2
        && word.starts_with(|c: char| c.is_ascii_lowercase())
        && word.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// Formats a list the way the shell original printed one: space separated, trailing space.
pub(crate) fn render(words: &[String]) -> String {
    let mut out = String::new();
    for word in words {
        out.push_str(word);
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        accepted_screens, appearances, case_arm_labels, is_locale, is_screen_name,
        linux_store_screens, validate_set,
    };

    #[test]
    fn reads_the_set_belonging_to_the_named_parameter() {
        let ps =
            "[ValidateSet('en','nl')] [string] $Locale,\n[ValidateSet('a','b')] [string] $Screen\n";
        assert_eq!(validate_set(ps, "$Locale"), ["en", "nl"]);
        assert_eq!(validate_set(ps, "$Screen"), ["a", "b"]);
    }

    #[test]
    fn an_absent_set_parses_to_nothing_rather_than_guessing() {
        // The caller treats empty as a failure; guessing would make the check blind.
        assert!(validate_set("no sets here\n", "$Locale").is_empty());
    }

    #[test]
    fn reads_case_arm_labels_but_not_the_commands_inside() {
        let body = "showcase_marker_for() {\n  case $1 in\n  en|nl)\n    printf 'x'\n    ;;\n  \
                    de)\n    echo hi\n  esac\n";
        let labels = case_arm_labels(body);
        assert!(labels.contains(&"en".to_owned()));
        assert!(labels.contains(&"de".to_owned()));
        assert!(!labels.contains(&"printf".to_owned()));
    }

    #[test]
    fn reads_the_linux_store_screen_arm() {
        let sh = "store_screens_for() {\n  case $1 in\n  linux) printf '%s\\n' mailbox reading \
                  invitation ;;\n  esac\n}\n";
        assert_eq!(
            linux_store_screens(sh),
            ["invitation", "mailbox", "reading"]
        );
    }

    #[test]
    fn the_printf_command_is_not_a_screen_name() {
        // `printf` is shaped exactly like a screen name, so a parser that keeps it reports a
        // screen the client "cannot reach" that was never offered.
        let sh = "store_screens_for() {\n  linux) printf '%s\\n' list reply ;;\n}\n";
        assert_eq!(linux_store_screens(sh), ["list", "reply"]);
    }

    #[test]
    fn accepted_screens_come_from_the_ok_arms_only() {
        let rs = "fn parse_screen(name: Option<&str>) -> Result<X, Y> {\n    match name {\n        \
                  Some(\"mailbox\") => Ok(ShowcaseScreen::Mailbox),\n        Some(\"invitation\") \
                  => Err(()),\n    }\n}\n";
        let accepted = accepted_screens(rs);
        assert!(accepted.contains(&"mailbox".to_owned()));
        // The refused name is present in the file, and must not be read as reachable.
        assert!(!accepted.contains(&"invitation".to_owned()));
    }

    #[test]
    fn appearance_words_survive_the_escaped_newline() {
        let sh = "appearances_for() {\n  # dark is mentioned in this comment\n  printf '%s\\n' \
                  light\\ndark\n}\n";
        let found = appearances(sh);
        assert!(found.contains(&"light".to_owned()));
        assert!(found.contains(&"dark".to_owned()));
    }

    #[test]
    fn shapes_reject_the_obvious_non_members() {
        assert!(is_locale("en"));
        assert!(!is_locale("eng"));
        assert!(is_screen_name("setup-detected"));
        assert!(!is_screen_name("Mailbox"));
    }
}
