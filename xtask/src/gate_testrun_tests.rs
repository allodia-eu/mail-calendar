use std::path::{Path, PathBuf};

use super::{package_of, read_build};

const MESSAGES: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///w/crates/mailcal-app#0.17.0","manifest_path":"/w/crates/mailcal-app/Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"mailcal_app"},"profile":{"test":true},"filenames":["/w/target/debug/deps/mailcal_app-1"],"executable":"/w/target/debug/deps/mailcal_app-1","fresh":true}
{"reason":"compiler-artifact","package_id":"path+file:///w/crates/mailcal-app#0.17.0","manifest_path":"/w/crates/mailcal-app/Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"mailcal_app"},"profile":{"test":false},"filenames":["/w/target/debug/deps/libmailcal_app-2.rlib"],"executable":null,"fresh":true}
{"reason":"compiler-artifact","package_id":"path+file:///w/crates/mailcal-bindings#0.17.0","manifest_path":"/w/crates/mailcal-bindings/Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"mailcal_bindings"},"profile":{"test":false},"filenames":["/w/target/debug/deps/libmailcal_bindings-4.rlib"],"executable":null,"fresh":false}
{"reason":"compiler-artifact","package_id":"path+file:///w/crates/mailcal-ffi#0.17.0","manifest_path":"/w/crates/mailcal-ffi/Cargo.toml","target":{"kind":["cdylib"],"crate_types":["cdylib"],"name":"mailcal_bindings"},"profile":{"test":false},"filenames":["/w/target/debug/libmailcal_bindings.so"],"executable":null,"fresh":false}
{"reason":"compiler-artifact","package_id":"path+file:///w/crates/mailcal-mcp-shim#0.17.0","manifest_path":"/w/crates/mailcal-mcp-shim/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"mailcal-mcp"},"profile":{"test":false},"filenames":["/w/target/debug/mailcal-mcp"],"executable":"/w/target/debug/mailcal-mcp","fresh":false}
{"reason":"compiler-artifact","package_id":"path+file:///w/crates/mailcal-mcp-shim#0.17.0","manifest_path":"/w/crates/mailcal-mcp-shim/Cargo.toml","target":{"kind":["test"],"crate_types":["bin"],"name":"relay"},"profile":{"test":true},"filenames":["/w/target/debug/deps/relay-3"],"executable":"/w/target/debug/deps/relay-3","fresh":false}
{"reason":"build-finished","success":true}
"#;

#[test]
fn a_build_yields_its_test_binaries_its_executables_and_the_bindings_library() {
    let build = read_build(MESSAGES).unwrap();
    let tests: Vec<(&str, &Path)> = build
        .tests
        .iter()
        .map(|t| (t.package.as_str(), t.path.as_path()))
        .collect();
    assert_eq!(
        tests,
        [
            (
                "mailcal-app",
                Path::new("/w/target/debug/deps/mailcal_app-1")
            ),
            (
                "mailcal-mcp-shim",
                Path::new("/w/target/debug/deps/relay-3")
            ),
        ]
    );
    assert_eq!(
        build.cdylib,
        Some(PathBuf::from("/w/target/debug/libmailcal_bindings.so"))
    );
    assert_eq!(
        build.bins.get("mailcal-mcp-shim").map(Vec::as_slice),
        Some(
            &[(
                "mailcal-mcp".to_owned(),
                PathBuf::from("/w/target/debug/mailcal-mcp")
            )][..]
        )
    );
}

#[test]
fn a_package_id_names_its_package_in_either_form() {
    assert_eq!(
        package_of("path+file:///w/crates/mailcal-app#0.17.0"),
        ("mailcal-app".to_owned(), "0.17.0".to_owned())
    );
    assert_eq!(
        package_of("path+file:///w/clients/linux#mailcal-linux@0.17.0"),
        ("mailcal-linux".to_owned(), "0.17.0".to_owned())
    );
}

#[test]
fn a_line_that_is_not_json_is_an_error_not_an_empty_build() {
    assert!(read_build("{\"reason\": \n").is_err());
}
