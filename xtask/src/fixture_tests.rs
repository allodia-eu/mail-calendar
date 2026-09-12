//! Fixture tests for the checks that search a whole tree.
//!
//! Each builds a throwaway git repository, writes sources into it, and asserts the verdict. A check
//! that only ever runs over this repository proves nothing about what it would say to a tree that
//! breaks the rule, and every rule here exists because something once broke it silently.
//!
//! This file holds the values the rules forbid, so the public-hygiene check excludes it by name,
//! the way it excludes its own pattern list.

use crate::{desktop_handoff, fixture::Repo, license_dir, portal_runtime, public_hygiene};

/// A manifest with a `default-members` list that does not name the closed directory.
const CLEAN_MANIFEST: &str =
    "[workspace]\nmembers = [\"crates/*\"]\ndefault-members = [\n  \"crates/x\",\n]\n";

// ---- the desktop hand-off rule ----------------------------------------------------------------

const PORTAL_SHAPED: &str = r"
fn open_it(uri: &str) {
    gtk::UriLauncher::new(uri).launch(None::<&gtk::Window>, gio::Cancellable::NONE, |_| ());
}
";

#[test]
fn portal_shaped_code_passes() {
    let repo = Repo::new(&[("clients/linux/src/ui.rs", PORTAL_SHAPED)]);
    assert_eq!(desktop_handoff::run(repo.root()), Ok(true));
}

#[test]
fn the_database_launcher_is_caught() {
    let source = "fn go() { gio::AppInfo::launch_default_for_uri(uri, None).unwrap(); }";
    let repo = Repo::new(&[("clients/linux/src/ui.rs", source)]);
    assert_eq!(desktop_handoff::run(repo.root()), Ok(false));
}

#[test]
fn the_pre_portal_spelling_is_caught() {
    let repo = Repo::new(&[(
        "clients/linux/src/ui.rs",
        "fn go() { gtk::show_uri(w, uri, 0); }",
    )]);
    assert_eq!(desktop_handoff::run(repo.root()), Ok(false));
}

#[test]
fn the_same_call_outside_the_linux_client_is_not_searched() {
    // The rule is about the sandboxed client. Another platform's source is not its business.
    let repo = Repo::new(&[
        ("clients/linux/src/ui.rs", PORTAL_SHAPED),
        (
            "crates/other/src/lib.rs",
            "fn go() { gtk::show_uri(w, uri, 0); }",
        ),
    ]);
    assert_eq!(desktop_handoff::run(repo.root()), Ok(true));
}

// ---- the one shared Tokio runtime --------------------------------------------------------------

#[test]
fn taking_the_shared_runtime_passes() {
    let source = "fn notify() { let Some(rt) = crate::host_runtime::shared() else { return; }; }";
    let repo = Repo::new(&[("clients/linux/src/notify.rs", source)]);
    assert_eq!(portal_runtime::run(repo.root()), Ok(true));
}

#[test]
fn a_runtime_built_by_a_service_is_caught() {
    let source = "fn notify() { let rt = runtime::Builder::new_current_thread().build(); }";
    let repo = Repo::new(&[("clients/linux/src/notify.rs", source)]);
    assert_eq!(portal_runtime::run(repo.root()), Ok(false));
}

#[test]
fn the_owner_of_the_only_runtime_may_build_one() {
    let source = "pub fn shared() { Builder::new_multi_thread().enable_all().build() }";
    let repo = Repo::new(&[("clients/linux/src/host_runtime.rs", source)]);
    assert_eq!(portal_runtime::run(repo.root()), Ok(true));
}

#[test]
fn test_code_may_build_its_own() {
    // The nesting guard in the secure store needs two distinct runtimes to prove anything at all.
    let source = "fn notify() {}\n\n#[cfg(test)]\nmod tests {\n    fn rt() { \
                  Builder::new_current_thread().build() }\n}\n";
    let repo = Repo::new(&[("clients/linux/src/notify.rs", source)]);
    assert_eq!(portal_runtime::run(repo.root()), Ok(true));
    // ...but only below the marker: the same line above it is production code.
    let early = "fn notify() { Builder::new_current_thread().build(); }\n\n#[cfg(test)]\nmod tests \
                 {}\n";
    let repo = Repo::new(&[("clients/linux/src/notify.rs", early)]);
    assert_eq!(portal_runtime::run(repo.root()), Ok(false));
}

#[test]
fn a_tests_file_is_test_code_throughout() {
    let source = "fn rt() { Builder::new_multi_thread().build() }";
    let repo = Repo::new(&[("clients/linux/src/store_tests.rs", source)]);
    assert_eq!(portal_runtime::run(repo.root()), Ok(true));
}

// ---- what the public repository must not carry ------------------------------------------------

#[test]
fn a_clean_tree_passes_public_hygiene() {
    let repo = Repo::new(&[(
        "docs/thing.md",
        "A sentence that names nobody and nothing.\n",
    )]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

#[test]
fn an_issue_reference_is_caught() {
    let repo = Repo::new(&[("docs/thing.md", "Fixed the ordering (#4321) last week.\n")]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn the_standin_issue_numbers_are_allowed() {
    // A few places have to show an issue-shaped token in order to forbid one.
    let repo = Repo::new(&[(
        "docs/thing.md",
        "The banned shape looks like (#1234) or (#1281).\n",
    )]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

#[test]
fn a_longer_number_beginning_with_a_standin_is_still_caught() {
    let repo = Repo::new(&[("docs/thing.md", "See issue #12345 for the rest.\n")]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn the_public_engine_tracker_may_be_cited() {
    let line = "Tracked in email-calendar-sync-engine issue #77.\n";
    let repo = Repo::new(&[("docs/thing.md", line)]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

#[test]
fn a_pointer_at_a_repository_the_reader_cannot_open_is_caught() {
    let repo = Repo::new(&[(
        "docs/thing.md",
        "The store push lives in the release repository.\n",
    )]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn the_category_rather_than_the_pointer_is_left_alone() {
    // "*a* private repository" is a category in an argument, which is how the pledge promises no
    // build step reaches one. It is the article that makes the rule decidable by a search.
    let repo = Repo::new(&[(
        "docs/thing.md",
        "No build step in this tree reaches a private repository.\n",
    )]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

#[test]
fn a_capitalised_plan_phase_is_caught() {
    let repo = Repo::new(&[(
        "crates/x/src/lib.rs",
        "// Display-only until Phase B lands.\n",
    )]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn the_domains_own_lowercase_phase_is_left_alone() {
    // A gesture's propagation phase and a build phase are not a plan's phases.
    let source = "// The controller runs in the capture phase, before the entry claims the key.\n";
    let repo = Repo::new(&[("crates/x/src/lib.rs", source)]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

#[test]
fn a_personal_identifier_is_caught() {
    let repo = Repo::new(&[("crates/x/tests/fixture.rs", "let to = \"Dennis\";\n")]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn the_licence_documents_may_name_a_party() {
    // An eenmanszaak has no legal personality of its own, so a licence naming only the brand would
    // bind nobody.
    let repo = Repo::new(&[("CLA.md", "This agreement is with Dennis Ameling.\n")]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

#[test]
fn a_store_reservation_is_caught() {
    let repo = Repo::new(&[("docs/thing.md", "The team id is X98DRMUM3J.\n")]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn the_internal_test_account_is_caught() {
    let repo = Repo::new(&[("docs/thing.md", "Sign in as someone@allodia.e2e first.\n")]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(false));
}

#[test]
fn a_path_this_check_does_not_read_is_not_searched() {
    // A working plan is where a phase name means something, and it is excluded outright.
    let repo = Repo::new(&[(
        "docs/some-plan.md",
        "Phase C ships the editor. See (#4321).\n",
    )]);
    assert_eq!(public_hygiene::run(repo.root()), Ok(true));
}

// ---- the closed directory the default build must not need -------------------------------------

#[test]
fn a_clean_tree_passes_the_licence_rule() {
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("crates/x/Cargo.toml", "[package]\n"),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(true));
}

#[test]
fn the_closed_directory_in_default_members_is_caught() {
    let manifest =
        "[workspace]\ndefault-members = [\n  \"allodia_license/crates/allodia-license\",\n]\n";
    let repo = Repo::new(&[("Cargo.toml", manifest)]);
    assert_eq!(license_dir::run(repo.root()), Ok(false));
}

#[test]
fn a_manifest_with_no_default_members_is_caught() {
    // Without the list, every member is in the default build.
    let repo = Repo::new(&[("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n")]);
    assert_eq!(license_dir::run(repo.root()), Ok(false));
}

#[test]
fn a_crate_reaching_into_the_directory_is_caught() {
    let dependency = "[dependencies]\nallodia-license = { path = \
                      \"../../allodia_license/crates/allodia-license\" }\n";
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("crates/x/Cargo.toml", dependency),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(false));
}

#[test]
fn the_optional_dependency_seam_is_allowed() {
    // The line has to exist for a branded build to turn the directory on; `optional = true` is what
    // keeps it out of everyone else's.
    let seam = "[dependencies]\nallodia-license = { path = \
                \"../../allodia_license/crates/allodia-license\", optional = true }\n";
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("crates/x/Cargo.toml", seam),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(true));
}

#[test]
fn the_feature_switched_on_by_default_is_caught() {
    let manifest = "[features]\ndefault = [\"allodia-license\"]\n";
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("crates/x/Cargo.toml", manifest),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(false));
}

#[test]
fn drifted_licence_texts_are_caught() {
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("allodia_license/LICENSE.md", "One wording.\n"),
        (
            "LICENSES/LicenseRef-Allodia-1.0.txt",
            "A different wording.\n",
        ),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(false));
}

#[test]
fn identical_licence_texts_pass() {
    let text = "The same wording, in both places.\n";
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("allodia_license/LICENSE.md", text),
        ("LICENSES/LicenseRef-Allodia-1.0.txt", text),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(true));
}

#[test]
fn a_licence_text_present_only_once_is_caught() {
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        ("allodia_license/LICENSE.md", "Only here.\n"),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(false));
}

#[test]
fn a_client_readme_may_explain_the_seam() {
    // Prose should name the directory; a rule forbidding the word is one nobody could document it
    // under.
    let repo = Repo::new(&[
        ("Cargo.toml", CLEAN_MANIFEST),
        (
            "clients/linux/README.md",
            "See allodia_license/crates for the closed half.\n",
        ),
    ]);
    assert_eq!(license_dir::run(repo.root()), Ok(true));
}
