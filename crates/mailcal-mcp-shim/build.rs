//! Injects the app's name from the brand (`docs/branding.md`), for the answer the relay gives when
//! the app cannot be reached.

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match mailcal_l10n::brand_value(&root, "MAILCAL_APP_NAME") {
        Ok(name) => println!("cargo:rustc-env=MAILCAL_APP_NAME={name}"),
        Err(error) => panic!("{error}"),
    }
}
