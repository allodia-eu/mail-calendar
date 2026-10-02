use super::{Columns, linux_registry, run, served_names, windows_glyphs};
use crate::fixture::Repo;

const DOC: &str = "\
Prose with a `backticked` word outside any table.

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Reply | `arrowshape.turn.up.left` | `ic_reply` | `E97A` Reply | `mail-reply-sender-symbolic` |
| Settings: general | `gearshape` | `ic_settings` | — | `mailcal-cogged-wheel-symbolic` (IDK `cogged-wheel`) |

| Glyph | Linux |
|---|---|
| Swipe | `mailcal-cogged-wheel-symbolic` |
";

#[test]
fn every_platform_column_is_read_by_its_header() {
    let columns = Columns::parse(DOC);
    assert_eq!(
        columns.apple.into_iter().collect::<Vec<_>>(),
        ["arrowshape.turn.up.left", "gearshape"]
    );
    assert_eq!(
        columns.android.into_iter().collect::<Vec<_>>(),
        ["ic_reply", "ic_settings"]
    );
    assert_eq!(columns.windows.into_iter().collect::<Vec<_>>(), ["E97A"]);
    assert_eq!(
        columns.linux.into_iter().collect::<Vec<_>>(),
        [
            "mail-reply-sender-symbolic",
            "mailcal-cogged-wheel-symbolic"
        ],
        "the upstream name noted beside a vendored glyph is not a Linux icon name"
    );
}

#[test]
fn the_registry_is_read_without_its_test_sentinel() {
    let source = "icons! {\n    A = \"mail-send-symbolic\";\n    B = A;\n}\n\
                  const RESOURCE_PATH: &str = \"/mailcal/icons\";\n\
                  mod tests { fn t() { has(\"mailcal-not-an-icon-symbolic\"); } }";
    assert_eq!(
        linux_registry(source).into_iter().collect::<Vec<_>>(),
        ["mail-send-symbolic"]
    );
}

#[test]
fn a_served_name_comes_from_the_alias_or_the_path() {
    let xml = r#"<file preprocess="xml-stripblanks">scalable/actions/mailcal-inbox-symbolic.svg</file>
<file alias="scalable/actions/mailcal-bell-symbolic.svg" preprocess="xml-stripblanks">idk/bell.svg</file>"#;
    assert_eq!(
        served_names(xml).into_iter().collect::<Vec<_>>(),
        ["mailcal-bell-symbolic", "mailcal-inbox-symbolic"]
    );
}

#[test]
fn every_spelling_of_a_windows_glyph_is_found() {
    let source = "<FontIcon Glyph=\"&#xE74D;\"/> Glyph = \"\\uE8b7\"; Glyph = \"\u{E787}\";\n\
                  new SymbolIcon { Symbol = Symbol.Delete }; <SymbolIcon Symbol=\"Important\"/>\n\
                  SwipeActionSymbol.Nothing; \"\\u00e9\" &#x20AC;";
    assert_eq!(
        windows_glyphs(source).into_iter().collect::<Vec<_>>(),
        ["E74D", "E787", "E8B7", "Symbol.Delete", "Symbol.Important"],
        "an ordinary escape, an HTML entity and a word ending in Symbol are not glyphs"
    );
}

const REGISTRY: &str = "icons! {\n    SEND = \"mail-send-symbolic\";\n    \
                        GENERAL = \"mailcal-cogged-wheel-symbolic\";\n}\n";

const GRESOURCE: &str = r#"<gresources><gresource prefix="/mailcal/icons">
<file alias="scalable/actions/mailcal-cogged-wheel-symbolic.svg">idk/cogged-wheel.svg</file>
</gresource></gresources>"#;

const TREE_DOC: &str = "\
| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Send | `paperplane` | `ic_send` | `E724` Send | `mail-send-symbolic` |
| General | `gearshape` | — | `Symbol.Setting` | `mailcal-cogged-wheel-symbolic` |
";

fn tree(overrides: &[(&'static str, &'static str)]) -> Repo {
    let mut files = vec![
        ("docs/icons.md", TREE_DOC),
        ("clients/linux/src/ui/icons.rs", REGISTRY),
        (
            "clients/linux/src/ui/row.rs",
            "fn row() { image(icons::SEND); }",
        ),
        ("clients/linux/icons/mailcal.gresource.xml", GRESOURCE),
        ("clients/linux/icons/idk/cogged-wheel.svg", "<svg/>"),
        (
            "REUSE.toml",
            "[[annotations]]\npath = \"clients/linux/icons/idk/**\"\nSPDX-License-Identifier = \"CC0-1.0\"\n",
        ),
        (
            "clients/android/app/src/main/res/drawable/ic_send.xml",
            "<vector/>",
        ),
        (
            "clients/windows/Mailcal/Views/Send.xaml",
            "<FontIcon Glyph=\"&#xE724;\"/><SymbolIcon Symbol=\"Setting\"/>",
        ),
        (
            "clients/apple/Sources/Row.swift",
            "Label(\"Send\", systemImage: \"paperplane\"); return \"gearshape\"",
        ),
    ];
    for (path, content) in overrides {
        files.retain(|(existing, _)| existing != path);
        files.push((path, content));
    }
    Repo::new(&files)
}

#[test]
fn a_tree_the_doc_describes_passes() {
    let repo = tree(&[]);
    assert_eq!(run(repo.root()), Ok(true));
}

#[test]
fn an_icon_name_outside_the_registry_is_caught() {
    let repo = tree(&[(
        "clients/linux/src/ui/row.rs",
        "fn row() { image(\"mail-send-symbolic\"); }",
    )]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_registry_name_the_doc_omits_is_caught() {
    let repo = tree(&[(
        "clients/linux/src/ui/icons.rs",
        "icons! {\n    SEND = \"mail-send-symbolic\";\n    \
         GENERAL = \"mailcal-cogged-wheel-symbolic\";\n    ADD = \"list-add-symbolic\";\n}\n",
    )]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_bundled_name_the_bundle_does_not_serve_is_caught() {
    let repo = tree(&[(
        "clients/linux/icons/mailcal.gresource.xml",
        "<gresources><gresource prefix=\"/mailcal/icons\"></gresource></gresources>",
    )]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_vendored_file_the_bundle_leaves_out_is_caught() {
    let repo = tree(&[("clients/linux/icons/idk/bell.svg", "<svg/>")]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_drawable_the_doc_omits_is_caught() {
    let repo = tree(&[(
        "clients/android/app/src/main/res/drawable/ic_reply.xml",
        "<vector/>",
    )]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_windows_glyph_the_doc_omits_is_caught() {
    let repo = tree(&[(
        "clients/windows/Mailcal/Views/Delete.cs",
        "var glyph = \"\\uE74D\";",
    )]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn an_sf_symbol_nothing_draws_is_caught() {
    let repo = tree(&[(
        "clients/apple/Sources/Row.swift",
        "Label(\"Send\", systemImage: \"paperplane\")",
    )]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn the_vendored_directory_must_keep_its_own_licence() {
    let repo = tree(&[("REUSE.toml", "version = 1\n")]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_glyph_in_a_file_at_the_projects_top_level_is_found() {
    let repo = tree(&[(
        "clients/windows/Mailcal/MainWindow.xaml",
        "<FontIcon Glyph=\"&#xE715;\"/>",
    )]);
    assert_eq!(
        run(repo.root()),
        Ok(false),
        "E715 is drawn and the doc omits it"
    );
}
