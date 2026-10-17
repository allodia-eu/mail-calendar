//! The readers behind `check-automation-ids`: which AutomationIds the Windows app's XAML and C#
//! give its elements, and which ones the UI suites' PowerShell looks up.
//!
//! None of them parses its language. Each reads only as much as the check needs and keeps its
//! place across what would otherwise derail it: a comment holding an id, a quote inside a string,
//! a string inside an interpolation hole.

/// An id a suite looks up.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Wanted {
    /// Written out whole.
    Exact(String),
    /// The text before a variable, as in `"AccountUse_$use"`.
    Prefix(String),
}

/// What the app's C# can make an id from.
#[derive(Debug, Default)]
pub(crate) struct CsLiterals {
    /// Every plain string literal: an id often reaches `SetAutomationId` through a parameter.
    pub(crate) exact: Vec<String>,
    /// The text before the first hole of an interpolated literal, as in `$"AccountUse_{use}"`.
    pub(crate) prefixes: Vec<String>,
    /// A literal appended to a name inside `SetAutomationId`, as in `automationId + "Server"`.
    pub(crate) suffixes: Vec<String>,
}

/// The ids a XAML file gives its elements.
///
/// An element's `AutomationProperties.AutomationId` is its id. Without one, UI Automation reports
/// its `x:Name` instead; with one, the name is hidden, which is why the two are read per element
/// rather than collected separately. A bound id is no literal, and still hides the name.
pub(crate) fn xaml_ids(text: &str) -> Vec<String> {
    let text = without_xml_comments(text);
    let mut ids: Vec<String> = Vec::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let end = tag_end(rest);
        let tag = &rest[..end];
        rest = &rest[end..];
        if tag.starts_with(['/', '!', '?']) {
            continue;
        }
        let attributes = attributes(tag);
        let value = |name: &str| attributes.iter().find(|(n, _)| *n == name).map(|(_, v)| *v);
        match value("AutomationProperties.AutomationId") {
            Some(id) if !id.starts_with('{') => ids.push(id.to_owned()),
            Some(_) => {}
            None => {
                if let Some(name) = value("x:Name").or_else(|| value("Name")) {
                    ids.push(name.to_owned());
                }
            }
        }
    }
    ids
}

/// `text` with every `<!-- … -->` removed, so a commented-out element names nothing.
fn without_xml_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = rest[start + 4..]
            .find("-->")
            .map_or("", |end| &rest[start + 4 + end + 3..]);
    }
    out.push_str(rest);
    out
}

/// Where the tag that `rest` starts inside ends: its first `>` outside a quoted value.
fn tag_end(rest: &str) -> usize {
    let mut quote: Option<char> = None;
    for (at, c) in rest.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return at,
            Some(_) | None => {}
        }
    }
    rest.len()
}

/// A tag's `name="value"` pairs. The element's own name, which has no value, is skipped.
fn attributes(tag: &str) -> Vec<(&str, &str)> {
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    let mut rest = tag;
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].split_whitespace().last().unwrap_or("");
        let after = rest[eq + 1..].trim_start();
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            rest = &rest[eq + 1..];
            continue;
        };
        let body = &after[1..];
        let close = body.find(quote).unwrap_or(body.len());
        pairs.push((name, &body[..close]));
        rest = body.get(close + 1..).unwrap_or("");
    }
    pairs
}

/// Adds what one C# file can make an id from to `out`.
pub(crate) fn cs_literals(text: &str, out: &mut CsLiterals) {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'/' if bytes.get(at + 1) == Some(&b'/') => at = skip_past(bytes, at, b"\n"),
            b'/' if bytes.get(at + 1) == Some(&b'*') => at = skip_past(bytes, at + 2, b"*/"),
            b'\'' => at = char_literal_end(bytes, at),
            b'$' | b'@' | b'"' => {
                let prefix = bytes[at..]
                    .iter()
                    .take_while(|b| **b == b'$' || **b == b'@')
                    .count();
                if bytes.get(at + prefix) == Some(&b'"') {
                    let marks = &bytes[at..at + prefix];
                    at = string_literal(text, at + prefix, marks, out);
                } else {
                    at += prefix.max(1);
                }
            }
            _ => at += 1,
        }
    }
    for line in text
        .lines()
        .filter(|line| line.contains("SetAutomationId("))
    {
        let mut rest = line;
        while let Some(plus) = rest.find("+ \"") {
            let body = &rest[plus + 3..];
            if let Some(close) = body.find('"')
                && body[close + 1..].starts_with(')')
            {
                out.suffixes.push(body[..close].to_owned());
            }
            rest = body;
        }
    }
}

/// The index just past the first `end` at or after `from`, or the end of `bytes`.
fn skip_past(bytes: &[u8], from: usize, end: &[u8]) -> usize {
    bytes[from..]
        .windows(end.len())
        .position(|w| w == end)
        .map_or(bytes.len(), |p| from + p + end.len())
}

/// The index just past a character literal starting at `at`, so `'"'` opens no string.
fn char_literal_end(bytes: &[u8], at: usize) -> usize {
    let mut i = at + 1;
    if bytes.get(i) == Some(&b'\\') {
        i += 2;
    }
    while i < bytes.len() && bytes[i] != b'\'' && bytes[i] != b'\n' {
        i += 1;
    }
    i + 1
}

/// Reads the string literal whose opening quote is at `quote`, records it, and returns the index
/// just past it. `marks` are the `$` and `@` written before the quote.
fn string_literal(text: &str, quote: usize, marks: &[u8], out: &mut CsLiterals) -> usize {
    let bytes = text.as_bytes();
    let quotes = bytes[quote..].iter().take_while(|b| **b == b'"').count();
    if quotes >= 3 {
        // A raw literal holds script or JSON, never an id; only its end matters.
        let fence = vec![b'"'; quotes];
        return skip_past(bytes, quote + quotes, &fence);
    }
    let interpolated = marks.contains(&b'$');
    let verbatim = marks.contains(&b'@');
    let start = quote + 1;
    let mut i = start;
    let mut first_hole: Option<usize> = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if !verbatim => i += 2,
            b'"' if verbatim && bytes.get(i + 1) == Some(&b'"') => i += 2,
            b'"' => break,
            b'{' if interpolated && bytes.get(i + 1) == Some(&b'{') => i += 2,
            b'{' if interpolated => {
                first_hole.get_or_insert(i);
                i = hole_end(bytes, i);
            }
            _ => i += 1,
        }
    }
    let end = i.min(bytes.len());
    match first_hole {
        // A hole at the very start fixes nothing about the id, so it would match every lookup.
        Some(hole) if hole > start => out.prefixes.push(text[start..hole].to_owned()),
        Some(_) => {}
        None => out.exact.push(text[start..end].to_owned()),
    }
    end + 1
}

/// The index just past the interpolation hole opening at `open`, stepping over strings inside it.
fn hole_end(bytes: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// The functions in a suite file whose first parameter is an AutomationId: `$AutomationId`, or
/// `$Id`, which every helper here uses for one. A call passes it first, positionally.
pub(crate) fn id_helpers(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut helpers: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = strip_prefix_ignore_case(line.trim_start(), "function ") else {
            continue;
        };
        let name: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let body = lines[index..lines.len().min(index + 6)].join("\n");
        let Some(param) = body.to_ascii_lowercase().find("param(") else {
            continue;
        };
        let first: String = body[param..]
            .split('$')
            .nth(1)
            .unwrap_or("")
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !name.is_empty()
            && (first.eq_ignore_ascii_case("AutomationId") || first.eq_ignore_ascii_case("Id"))
        {
            helpers.push(name);
        }
    }
    helpers
}

/// Every id a suite file looks up, with its line number, given the helpers that take one first.
pub(crate) fn lookups(text: &str, helpers: &[String]) -> Vec<(usize, Wanted)> {
    let mut found: Vec<(usize, Wanted)> = Vec::new();
    let mut in_block = false;
    for (index, line) in text.lines().enumerate() {
        let code = code_of(line, &mut in_block);
        let lower = code.to_ascii_lowercase();
        let mut record = |after: usize| {
            for wanted in quoted_after(&code[after..]) {
                found.push((index + 1, wanted));
            }
        };
        for at in find_all(&lower, "-automationid") {
            record(at + "-automationid".len());
        }
        for at in find_all(&lower, ".automationid") {
            let rest = lower[at + ".automationid".len()..].trim_start();
            for operator in ["-eq", "-ceq", "-ne", "-in"] {
                if let Some(after) = rest.strip_prefix(operator) {
                    record(lower.len() - after.len());
                }
            }
        }
        for helper in helpers {
            let helper = helper.to_ascii_lowercase();
            for at in find_all(&lower, &helper) {
                let before = lower[..at].chars().next_back();
                let after = &lower[at + helper.len()..];
                if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
                    || !after.starts_with([' ', '\t'])
                    || lower[..at].trim_end().ends_with("function")
                {
                    continue;
                }
                let args = after.trim_start();
                let args = args.strip_prefix("-id").map_or(args, str::trim_start);
                record(lower.len() - args.len());
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The code on a PowerShell line: what is left once comments are removed. `in_block` carries a
/// `<# … #>` block across lines.
fn code_of(line: &str, in_block: &mut bool) -> String {
    let mut code = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if *in_block {
            if c == '#' && chars.peek() == Some(&'>') {
                chars.next();
                *in_block = false;
            }
            continue;
        }
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '`' => {
                code.push(c);
                if let Some(next) = chars.next() {
                    code.push(next);
                }
                continue;
            }
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == '<' && chars.peek() == Some(&'#') => {
                chars.next();
                *in_block = true;
                continue;
            }
            None if c == '#' => break,
            Some(_) | None => {}
        }
        code.push(c);
    }
    code
}

/// The quoted ids at the start of `rest`: one string, or each string of an `@( … )` list.
fn quoted_after(rest: &str) -> Vec<Wanted> {
    let rest = rest.trim_start();
    if let Some(list) = rest.strip_prefix("@(") {
        let list = &list[..list.find(')').unwrap_or(list.len())];
        return list
            .split(',')
            .filter_map(|item| quoted(item.trim()))
            .collect();
    }
    quoted(rest).into_iter().collect()
}

/// The id a string at the start of `text` names, if one starts there. Single quotes are literal;
/// double quotes interpolate, so only what comes before the first `$` is known.
fn quoted(text: &str) -> Option<Wanted> {
    let quote = text.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &text[1..];
    let body = &body[..body.find(quote)?];
    if quote == '"'
        && let Some(dollar) = body.find('$')
    {
        return (dollar > 0).then(|| Wanted::Prefix(body[..dollar].to_owned()));
    }
    (!body.is_empty()).then(|| Wanted::Exact(body.to_owned()))
}

/// Every index at which `needle` starts in `haystack`.
fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    haystack.match_indices(needle).map(|(at, _)| at).collect()
}

/// `text` without `prefix`, compared without regard to ASCII case.
fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &text[prefix.len()..])
}

/// Every id a C# suite looks up, with its line number: a quoted id passed to `Filter.ById`, or set
/// as a filter's `Id`.
pub(crate) fn cs_lookups(text: &str) -> Vec<(usize, Wanted)> {
    let mut found: Vec<(usize, Wanted)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for marker in ["ById(\"", "Id = \""] {
            for (at, _) in line.match_indices(marker) {
                let rest = &line[at + marker.len()..];
                if let Some(end) = rest.find('"')
                    && end > 0
                {
                    found.push((index + 1, Wanted::Exact(rest[..end].to_owned())));
                }
            }
        }
    }
    found
}
