//! Emits the Rust accessor module the Linux client compiles, into a directory of your choosing.
//!
//! The generator runs from `clients/linux/build.rs`, so on a host that cannot build the GTK client
//! there is otherwise no way to see what it produced, and a client calling an accessor the
//! catalog does not carry is the one blind-write error a grep can decide.
//!
//! `cargo run -p mailcal-l10n --example emit_rust -- <out-dir>`

fn main() {
    let out = std::env::args().nth(1).expect("usage: emit_rust <out-dir>");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/mailcal-l10n has a repository root");
    mailcal_l10n::generate(
        mailcal_l10n::Target::Rust,
        root,
        std::path::Path::new(&out),
        "",
    )
    .expect("the shared localization catalog generates Rust accessors");
    println!("wrote {out}/l10n.rs");
}
