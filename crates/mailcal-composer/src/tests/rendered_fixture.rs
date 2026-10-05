//! The renderer's half of the contract the editor's reader is held to.
//!
//! A message reopened in the composer is read back out of the HTML this crate rendered
//! (`clients/composer/src/read_html.ts`), and has to come back as the document it was rendered
//! from, or the composer's next save changes the user's draft. Neither side can check that alone,
//! so they share a fixture: this test holds each entry's `html` to what [`render`] writes for its
//! `document`, and the editor's suite holds its reader to turning that `html` back into the
//! `document`. A change to the renderer fails here first; `MAILCAL_BLESS=1` rewrites the fixture,
//! and the editor's suite then says whether the reader still follows.

use std::path::PathBuf;

use serde_json::Value;

use crate::{ComposerDocument, render};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../clients/composer/tests/fixtures/rendered.json")
}

#[test]
fn the_shared_fixture_holds_what_the_renderer_writes() {
    let path = fixture_path();
    let source = std::fs::read_to_string(&path).expect("the shared fixture is readable");
    let mut entries: Vec<Value> = serde_json::from_str(&source).expect("the fixture is JSON");
    let bless = std::env::var_os("MAILCAL_BLESS").is_some();
    let mut stale = Vec::new();
    for entry in &mut entries {
        let name = entry["name"].as_str().unwrap_or_default().to_owned();
        let document: ComposerDocument = serde_json::from_value(entry["document"].clone())
            .unwrap_or_else(|err| panic!("{name}: the document does not parse: {err}"));
        let html = render(&document)
            .unwrap_or_else(|err| panic!("{name}: the document does not render: {err:?}"))
            .html;
        if entry["html"].as_str() != Some(html.as_str()) {
            stale.push(name);
            entry["html"] = Value::String(html);
        }
    }
    if bless {
        let mut written = serde_json::to_string_pretty(&entries).expect("serialisable");
        written.push('\n');
        std::fs::write(&path, written).expect("the fixture is writable");
        return;
    }
    assert!(
        stale.is_empty(),
        "the renderer no longer writes what the fixture holds for {stale:?}; re-run with \
         MAILCAL_BLESS=1, then run the editor's suite (`bun test` in clients/composer)"
    );
}
