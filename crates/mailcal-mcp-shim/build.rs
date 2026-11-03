//! Injects the app's name and the relay's installed name from the brand (`docs/branding.md`), for
//! the answers the relay gives when it is run wrongly or the app cannot be reached.

use std::path::Path;

/// The brand values the relay names itself and the app by.
const BRAND_VARS: [&str; 2] = ["MAILCAL_APP_NAME", "MAILCAL_MCP_RELAY_NAME"];

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for key in BRAND_VARS {
        match mailcal_l10n::brand_value(&root, key) {
            Ok(value) => println!("cargo:rustc-env={key}={value}"),
            Err(error) => panic!("{error}"),
        }
    }
}
