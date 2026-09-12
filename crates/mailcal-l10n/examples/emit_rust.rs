//! Emits the Rust accessor module the Linux client compiles, into a directory of your choosing.
//!
//! The generator runs from `clients/linux/build.rs`, so on a host that cannot build the GTK client
//! there is otherwise no way to see what it produced, and a client calling an accessor the
//! catalog does not carry is the one blind-write error a grep can decide.
//!
//! `cargo run -p mailcal-l10n --example emit_rust -- <out-dir>`

fn main() {
    let out = std::env::args().nth(1).expect("usage: emit_rust <out-dir>");
    // Asked of the cargo running this, not of the compile that produced it. Every checkout shares
    // one build directory (`.cargo/config.toml`), so a cached example carries the absolute path of
    // whichever worktree built it first, and would read that tree's catalog while reporting the
    // accessors as this one's. The compiled value is the fallback, for running the binary
    // directly.
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    let manifest = std::path::Path::new(&manifest);
    let root = manifest
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
