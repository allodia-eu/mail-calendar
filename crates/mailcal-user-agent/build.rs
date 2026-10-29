//! Names the product for the `User-Agent` from the brand, by the rule every other build system
//! reads it by (`docs/branding.md`).

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match mailcal_l10n::user_agent_product(&root) {
        Ok(product) => println!("cargo:rustc-env=MAILCAL_USER_AGENT_PRODUCT={product}"),
        Err(error) => panic!("{error}"),
    }
}
