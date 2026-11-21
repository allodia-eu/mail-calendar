//! Reads the quote shapes this app writes from the message catalog, so the stripper that finds
//! them in sent mail moves when the copy does.
//!
//! Emits one `LocaleShapes` per catalog locale into `$OUT_DIR/catalog_shapes.rs`: the attribution
//! template ("On {date}, {sender} wrote:"), the forwarded-message label and the header labels of
//! the line-and-header quote style. Read-only use of the catalog: the core still produces no
//! localised text.

use std::{env, fmt::Write as _, fs, path::PathBuf};

/// The catalog keys read, in the order `LocaleShapes` holds them.
const HEADER_KEYS: [&str; 5] = [
    "quote_from",
    "quote_sent",
    "quote_to",
    "quote_cc",
    "quote_subject",
];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest
        .parent()
        .and_then(|crates| crates.parent())
        .expect("crates/mailcal-ai has a repository root");
    let settings = root.join("project.inlang/settings.json");
    let messages = root.join("messages");
    println!("cargo:rerun-if-changed={}", settings.display());
    println!("cargo:rerun-if-changed={}", messages.display());

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).expect("inlang settings"))
            .expect("inlang settings are JSON");
    let locales = settings["locales"]
        .as_array()
        .expect("inlang settings list the locales");

    let mut out = String::from("&[\n");
    for locale in locales {
        let locale = locale.as_str().expect("a locale is a string");
        let path = messages.join(format!("{locale}.json"));
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("a catalog file"))
                .expect("a catalog file is JSON");
        let text = |key: &str| {
            catalog[key]
                .as_str()
                .unwrap_or_else(|| panic!("{locale}.json has {key}"))
                .to_owned()
        };
        let headers: Vec<String> = HEADER_KEYS.iter().map(|key| text(key)).collect();
        writeln!(
            out,
            "    LocaleShapes {{ locale: {locale:?}, attribution: {:?}, forwarded: {:?}, headers: &{headers:?} }},",
            text("quote_attribution"),
            text("quote_forwarded"),
        )
        .expect("writing to a string");
    }
    out.push(']');

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    fs::write(out_dir.join("catalog_shapes.rs"), out).expect("writing the generated shapes");
}
