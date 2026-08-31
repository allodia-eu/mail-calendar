//! Generates the Linux client's typed localisation module from the shared catalog, and compiles
//! the bundled icons into a GResource.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use mailcal_l10n::Target;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|clients| clients.parent())
        .expect("clients/linux has a repository root");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));

    mailcal_l10n::generate(Target::Rust, repo_root, &out_dir, "")
        .expect("the shared localization catalog generates Rust accessors");
    compile_icons(&manifest_dir.join("icons"), &out_dir);
    stamp_build(repo_root, &manifest_dir);

    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("messages").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("project.inlang/settings.json").display()
    );
}

/// Stamps diagnostic logs with the source and build time without changing the marketing version.
fn stamp_build(repo_root: &Path, manifest_dir: &Path) {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let roots = [
        repo_root.join("Cargo.toml"),
        repo_root.join("Cargo.lock"),
        repo_root.join("VERSION"),
        repo_root.join("crates"),
        repo_root.join("messages"),
        repo_root.join("project.inlang"),
        repo_root.join("clients/composer/dist/editor.html"),
        manifest_dir.join("Cargo.toml"),
        manifest_dir.join("src"),
        manifest_dir.join("icons"),
    ];
    let mut files = Vec::new();
    for root in roots {
        collect_files(&root, &mut files);
    }
    files.sort_unstable();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        hash_bytes(
            &mut hash,
            path.strip_prefix(repo_root)
                .unwrap_or(&path)
                .as_os_str()
                .as_encoded_bytes(),
        );
        if let Ok(bytes) = fs::read(&path) {
            hash_bytes(&mut hash, &bytes);
        }
    }
    let epoch = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("build clock is after the Unix epoch")
                .as_secs()
        });
    println!("cargo:rustc-env=MAILCAL_BUILD_ID={hash:012x}.{epoch}");
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        files.push(path.to_owned());
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_files(&entry.path(), files);
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// Compiles the icon bundle the app registers at startup.
///
/// `glib-compile-resources` ships in `libglib2.0-dev-bin`, which `libgtk-4-dev` already pulls in,
/// so this asks nothing of a machine that can build the client at all; hence a direct call
/// rather than a crate that wraps the same binary.
fn compile_icons(icons: &Path, out_dir: &Path) {
    let manifest = icons.join("mailcal.gresource.xml");
    let target = out_dir.join("mailcal.gresource");
    let status = Command::new("glib-compile-resources")
        .arg(format!("--target={}", target.display()))
        .arg(format!("--sourcedir={}", icons.display()))
        .arg(&manifest)
        .status()
        .expect("glib-compile-resources (libglib2.0-dev-bin) builds the icon bundle");
    assert!(status.success(), "glib-compile-resources failed: {status}");
    println!("cargo:rerun-if-changed={}", icons.display());
}
