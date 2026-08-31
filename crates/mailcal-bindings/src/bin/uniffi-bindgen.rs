//! The UniFFI binding generator, invoked in library mode against the built cdylib:
//! `cargo run --bin uniffi-bindgen -- generate --library <dylib> --language <lang>`.
fn main() {
    uniffi::uniffi_bindgen_main();
}
