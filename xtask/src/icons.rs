//! Holds `docs/icons.md` to the icons the clients actually draw.
//!
//! The doc's mapping tables name every glyph per platform, and a table nobody checks describes the
//! tree it was written against. So each column is compared with its source: Linux's with the one
//! registry every name must live in, Android's with the drawables on disk, Windows' with the glyph
//! code points and `Symbol` values in the app. Apple's is checked one way only, that every symbol
//! the doc names is drawn somewhere, because an SF Symbol name in Swift is a plain string literal
//! and cannot be told apart from any other.

use std::{collections::BTreeSet, path::Path};

use crate::{
    git::{self, Kind},
    report::Report,
};

const DOC: &str = "docs/icons.md";
const LINUX_REGISTRY: &str = "clients/linux/src/ui/icons.rs";
const LINUX_SOURCES: &str = "clients/linux/src";
const GRESOURCE: &str = "clients/linux/icons/mailcal.gresource.xml";
const LINUX_ICONS: &str = "clients/linux/icons";
const IDK_DIR: &str = "clients/linux/icons/idk";
const ANDROID_DRAWABLES: &str = "clients/android/app/src/main/res/drawable";
const WINDOWS_APP: &str = "clients/windows/Mailcal";
const APPLE_SOURCES: &str = "clients/apple";

/// Runs the check. `Ok(true)` means the doc and the tree agree.
///
/// # Errors
///
/// Propagates a git or filesystem failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut report = Report::new();
    let doc = git::read(root, DOC)?;
    let columns = Columns::parse(&doc);

    linux(root, &columns.linux, &mut report)?;
    android(root, &columns.android, &mut report)?;
    windows(root, &columns.windows, &mut report)?;
    apple(root, &columns.apple, &mut report)?;

    if report.failed() {
        eprintln!("{DOC} and the icons the clients draw disagree:");
        report.emit();
        return Ok(false);
    }
    println!(
        "OK: {DOC} names every icon: {} Linux, {} Android, {} Windows, {} Apple.",
        columns.linux.len(),
        columns.android.len(),
        columns.windows.len(),
        columns.apple.len()
    );
    Ok(true)
}

/// The glyphs each platform column of the doc's mapping tables names, as backticked tokens.
#[derive(Debug, Default)]
pub(crate) struct Columns {
    pub(crate) apple: BTreeSet<String>,
    pub(crate) android: BTreeSet<String>,
    pub(crate) windows: BTreeSet<String>,
    pub(crate) linux: BTreeSet<String>,
}

impl Columns {
    /// Reads every table whose header names at least one platform. A row's cell is read by the
    /// header above it, so a table may carry any subset of the four columns in any order.
    pub(crate) fn parse(doc: &str) -> Self {
        let mut columns = Self::default();
        let mut header: Vec<String> = Vec::new();
        for line in doc.lines() {
            let line = line.trim();
            if !line.starts_with('|') {
                header.clear();
                continue;
            }
            let cells = cells(line);
            if header.is_empty() {
                header = cells.iter().map(|cell| cell.trim().to_owned()).collect();
                continue;
            }
            if cells
                .iter()
                .all(|cell| cell.trim().chars().all(|c| "-: ".contains(c)))
            {
                continue;
            }
            for (name, cell) in header.iter().zip(&cells) {
                let (set, shaped): (_, fn(&str) -> bool) = match name.as_str() {
                    "Apple" => (&mut columns.apple, is_sf_symbol),
                    "Android" => (&mut columns.android, is_drawable),
                    "Windows" => (&mut columns.windows, is_windows_glyph),
                    "Linux" => (&mut columns.linux, is_linux_name),
                    _ => continue,
                };
                set.extend(backticked(cell).filter(|token| shaped(token)));
            }
        }
        columns
    }
}

// A cell may carry a note beside its glyph (the upstream name of a vendored icon, say), so each
// column keeps only the tokens shaped like that platform's names.

fn is_sf_symbol(token: &str) -> bool {
    !token.is_empty()
        && token.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn is_drawable(token: &str) -> bool {
    token.strip_prefix("ic_").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    })
}

fn is_windows_glyph(token: &str) -> bool {
    let code = token.len() == 4
        && token
            .chars()
            .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c));
    let symbol = token.strip_prefix("Symbol.").is_some_and(|name| {
        name.starts_with(|c: char| c.is_ascii_uppercase())
            && name.chars().all(|c| c.is_ascii_alphanumeric())
    });
    code || symbol
}

fn is_linux_name(token: &str) -> bool {
    token.strip_suffix("-symbolic").is_some_and(|stem| {
        !stem.is_empty()
            && stem
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    })
}

fn cells(line: &str) -> Vec<&str> {
    let inner = line.trim_matches('|');
    inner.split('|').collect()
}

fn backticked(cell: &str) -> impl Iterator<Item = String> + '_ {
    cell.split('`').skip(1).step_by(2).map(str::to_owned)
}

/// Linux: one registry, the doc's column equal to it, and the bundle serving exactly the
/// `mailcal-…` names it lists.
fn linux(root: &Path, documented: &BTreeSet<String>, report: &mut Report) -> Result<(), String> {
    let stray = git::grep(
        root,
        Kind::Extended,
        "\"[a-z0-9-]+-symbolic\"",
        &[LINUX_SOURCES, &format!(":!{LINUX_REGISTRY}")],
    )?;
    for line in &stray.lines {
        report.note(format!(
            "an icon name outside {LINUX_REGISTRY}, where the test that it resolves cannot see it:\n    {line}"
        ));
    }

    let registry = linux_registry(&git::read(root, LINUX_REGISTRY)?);
    compare(report, "Linux", LINUX_REGISTRY, &registry, documented);

    let gresource = git::read(root, GRESOURCE)?;
    let served = served_names(&gresource);
    let bundled: BTreeSet<String> = registry
        .iter()
        .filter(|name| name.starts_with("mailcal-"))
        .cloned()
        .collect();
    for name in bundled.difference(&served) {
        report.note(format!(
            "{name} is in the registry and {GRESOURCE} does not serve it"
        ));
    }
    for name in served.difference(&bundled) {
        report.note(format!("{GRESOURCE} serves {name}, which nothing draws"));
    }

    let sources = source_files(&gresource);
    for file in git::listed(root, &[&format!("{LINUX_ICONS}/*.svg")])? {
        let relative = file.trim_start_matches(LINUX_ICONS).trim_start_matches('/');
        if !sources.contains(relative) {
            report.note(format!("{file} is not compiled into {GRESOURCE}"));
        }
    }

    let reuse = git::read(root, "REUSE.toml")?;
    if !reuse.contains(&format!("\"{IDK_DIR}/**\"")) || !reuse.contains("CC0-1.0") {
        report.note(format!(
            "REUSE.toml does not label {IDK_DIR}/** as CC0-1.0: the blanket would relabel the \
             icon-development-kit's files GPL"
        ));
    }
    Ok(())
}

/// The names the registry defines, test module excluded: its sentinel is a name no theme has, on
/// purpose.
pub(crate) fn linux_registry(source: &str) -> BTreeSet<String> {
    let body = source.split("mod tests").next().unwrap_or(source);
    quoted(body)
        .filter(|name| name.ends_with("-symbolic"))
        .collect()
}

fn quoted(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split('"').skip(1).step_by(2).map(str::to_owned)
}

/// The icon names the bundle serves: `scalable/actions/<name>.svg`, from either a plain entry or
/// an alias.
pub(crate) fn served_names(gresource: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in gresource.split("<file").skip(1) {
        let path = entry
            .split("alias=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .or_else(|| {
                entry
                    .split('>')
                    .nth(1)
                    .and_then(|rest| rest.split('<').next())
            });
        if let Some(name) = path
            .and_then(|path| path.rsplit('/').next())
            .and_then(|file| file.strip_suffix(".svg"))
        {
            names.insert(name.to_owned());
        }
    }
    names
}

/// The files the bundle reads, relative to its source directory.
fn source_files(gresource: &str) -> BTreeSet<String> {
    gresource
        .split("<file")
        .skip(1)
        .filter_map(|entry| entry.split('>').nth(1)?.split('<').next())
        .map(str::to_owned)
        .collect()
}

/// Android: the drawables on disk are the column.
fn android(root: &Path, documented: &BTreeSet<String>, report: &mut Report) -> Result<(), String> {
    let drawn: BTreeSet<String> = git::listed(root, &[&format!("{ANDROID_DRAWABLES}/ic_*.xml")])?
        .iter()
        .filter_map(|path| path.rsplit('/').next()?.strip_suffix(".xml"))
        .map(str::to_owned)
        .collect();
    compare(report, "Android", ANDROID_DRAWABLES, &drawn, documented);
    Ok(())
}

/// Windows: every Segoe glyph code point and `Symbol` value the app names.
///
/// The pathspecs are `Mailcal/*.cs`, never `Mailcal/**/*.cs`: git matches a plain `*` across
/// directories, and `**/` demands at least one, which skips `MainWindow.xaml` and every other file
/// at the project's top level.
fn windows(root: &Path, documented: &BTreeSet<String>, report: &mut Report) -> Result<(), String> {
    let mut drawn = BTreeSet::new();
    for file in git::listed(
        root,
        &[
            &format!("{WINDOWS_APP}/*.cs"),
            &format!("{WINDOWS_APP}/*.xaml"),
        ],
    )? {
        drawn.extend(windows_glyphs(&git::read(root, &file)?));
    }
    compare(report, "Windows", WINDOWS_APP, &drawn, documented);
    Ok(())
}

/// The glyphs one Windows source names: `&#xE74D;`, `""`, a raw private-use character, or a
/// `Symbol` enum value, each as the doc writes it (`E74D`, `Symbol.Delete`).
pub(crate) fn windows_glyphs(source: &str) -> BTreeSet<String> {
    let mut glyphs = BTreeSet::new();
    for (prefix, end) in [("&#x", ";"), ("\\u", "")] {
        for rest in source.split(prefix).skip(1) {
            let hex: String = rest.chars().take(4).collect();
            let terminated = end.is_empty() || rest[hex.len()..].starts_with(end);
            if let Ok(code) = u32::from_str_radix(&hex, 16)
                && hex.len() == 4
                && terminated
                && is_private_use(code)
            {
                glyphs.insert(hex.to_ascii_uppercase());
            }
        }
    }
    for c in source.chars() {
        if is_private_use(u32::from(c)) {
            glyphs.insert(format!("{:04X}", u32::from(c)));
        }
    }
    for (prefix, end) in [("Symbol.", ""), ("Symbol=\"", "\"")] {
        for (at, _) in source.match_indices(prefix) {
            let starts_word = !source[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric());
            let rest = &source[at + prefix.len()..];
            let name: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            let terminated = end.is_empty() || rest[name.len()..].starts_with(end);
            if starts_word && name.starts_with(|c: char| c.is_ascii_uppercase()) && terminated {
                glyphs.insert(format!("Symbol.{name}"));
            }
        }
    }
    glyphs
}

const fn is_private_use(code: u32) -> bool {
    matches!(code, 0xE000..=0xF8FF)
}

/// Apple: every symbol the doc names is a quoted literal somewhere in the Swift sources.
fn apple(root: &Path, documented: &BTreeSet<String>, report: &mut Report) -> Result<(), String> {
    for symbol in documented {
        let hits = git::grep(
            root,
            Kind::Fixed,
            &format!("\"{symbol}\""),
            &[
                &format!("{APPLE_SOURCES}/*.swift"),
                &format!(":!{APPLE_SOURCES}/*Tests/*"),
            ],
        )?;
        if hits.is_empty() {
            report.note(format!(
                "{DOC} names the SF Symbol {symbol}, which nothing in {APPLE_SOURCES} draws"
            ));
        }
    }
    Ok(())
}

fn compare(
    report: &mut Report,
    platform: &str,
    source: &str,
    drawn: &BTreeSet<String>,
    documented: &BTreeSet<String>,
) {
    for glyph in drawn.difference(documented) {
        report.note(format!(
            "{platform}: {glyph} is drawn ({source}) and missing from the {platform} column"
        ));
    }
    for glyph in documented.difference(drawn) {
        report.note(format!(
            "{platform}: the {platform} column names {glyph}, which {source} does not draw"
        ));
    }
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
