//! The UniFFI binding generator, invoked in library mode against a built cdylib:
//! `cargo run -p mailcal-bindgen-uniffi -- generate --library <cdylib> --language <lang>`.
fn main() {
    uniffi::uniffi_bindgen_main();
}
