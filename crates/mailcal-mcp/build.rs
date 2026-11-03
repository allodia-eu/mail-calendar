//! Injects how the server introduces itself, from the brand (`docs/branding.md`): what
//! `src/branding.rs` reads with `env!`.

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for key in [
        "MAILCAL_APP_NAME",
        "MAILCAL_MCP_SERVER_NAME",
        "MAILCAL_WEBSITE_URL",
    ] {
        match mailcal_l10n::brand_value(&root, key) {
            Ok(value) => println!("cargo:rustc-env={key}={value}"),
            Err(error) => panic!("{error}"),
        }
    }
}
