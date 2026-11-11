//! Tests for `check-test-agent`: its region reader, and its verdict over fixture trees.

use super::{debug_regions, run};
use crate::fixture::Repo;

const ENTRY: &str = "clients/windows/Mailcal/Services/TestAgent.cs";
const TREE: &str = "clients/windows/Mailcal/Services/TestAgent.Tree.cs";

const GOOD_ENTRY: &str = r#"// The agent.
using System.Diagnostics;
namespace X;
internal static partial class TestAgent
{
    /// <summary>Starts it.</summary>
    [Conditional("DEBUG")]
    internal static void Start(object window)
    {
#if DEBUG
        var pipe = System.Environment.GetEnvironmentVariable("MAILCAL_TEST_AGENT");
#endif
    }
#if DEBUG
    private static void Serve(string pipe) { }
#endif
}
"#;

const LINUX_MAIN: &str = "clients/linux/src/main.rs";
const LINUX_AGENT: &str = "clients/linux/src/agent.rs";
const GOOD_LINUX_MAIN: &str =
    "#[cfg(any(debug_assertions, feature = \"dev-harness\"))]\nmod agent;\nmod ui;\n";
const GOOD_LINUX_AGENT: &str = "//! The agent.\nconst SWITCH: &str = \"MAILCAL_TEST_AGENT\";\n";

const APPLE_ENTRY: &str =
    "clients/apple/Packages/MailcalKit/Sources/MailcalUI/TestAgent/TestAgent.swift";
const APPLE_MAC: &str =
    "clients/apple/Packages/MailcalKit/Sources/MailcalUI/TestAgent/TestAgentMac.swift";
const APPLE_SESSION: &str = "clients/apple/Packages/MailcalKit/Sources/MailcalUI/AppSession.swift";
const GOOD_APPLE_ENTRY: &str =
    "// The agent.\n\n#if DEBUG\nimport Foundation\nlet name = \"MAILCAL_TEST_AGENT\"\n#endif\n";
const GOOD_APPLE_MAC: &str =
    "// macOS.\n#if DEBUG && os(macOS)\nimport AppKit\nenum AgentPlatform {}\n#endif\n";
const GOOD_APPLE_SESSION: &str = "init() {\n    #if DEBUG\n    TestAgent.start()\n    #endif\n    // TestAgent.start() is debug-only.\n}\n";

const GOOD_TREE: &str = "// The tree.\n\n#if DEBUG\nusing System;\nnamespace X;\ninternal static partial class TestAgent { }\n#endif\n";

#[test]
fn a_line_knows_whether_it_is_in_a_debug_region() {
    let regions = debug_regions("a\n#if DEBUG\nb\n#if WINDOWS\nc\n#endif\n#else\nd\n#endif\ne\n");
    let inside: Vec<(bool, &str)> = regions
        .into_iter()
        .filter(|(_, line)| !line.is_empty())
        .collect();
    assert_eq!(
        inside,
        [
            (false, "a"),
            (true, "b"),
            (true, "c"),
            (false, "d"),
            (false, "e")
        ]
    );
}

#[test]
fn an_agent_held_out_of_release_passes() {
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
    ]);
    assert_eq!(run(repo.root()), Ok(true));
}

#[test]
fn an_entry_point_a_release_build_would_keep_fails() {
    let entry = GOOD_ENTRY.replace("    [Conditional(\"DEBUG\")]\n", "");
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (ENTRY, entry.as_str()),
        (TREE, GOOD_TREE),
    ]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn an_agent_file_with_code_outside_debug_fails() {
    let tree = GOOD_TREE.replace("#if DEBUG\n", "").replace("#endif\n", "");
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (ENTRY, GOOD_ENTRY),
        (TREE, tree.as_str()),
    ]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn the_switch_read_outside_debug_anywhere_in_the_app_fails() {
    let elsewhere = "namespace X;\nclass App { string s = System.Environment.GetEnvironmentVariable(\"MAILCAL_TEST_AGENT\"); }\n";
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
        ("clients/windows/Mailcal/App.xaml.cs", elsewhere),
    ]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_tree_without_the_agent_is_an_error_not_a_pass() {
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (TREE, GOOD_TREE),
    ]);
    assert!(run(repo.root()).is_err());
}

#[test]
fn a_linux_agent_module_without_its_gate_fails() {
    let repo = Repo::new(&[
        (LINUX_MAIN, "mod agent;\nmod ui;\n"),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
    ]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn the_linux_switch_read_outside_the_agent_fails() {
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (
            "clients/linux/src/ui.rs",
            "let socket = std::env::var(\"MAILCAL_TEST_AGENT\");\n",
        ),
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
    ]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_region_names_debug_with_a_platform_and_an_elseif_starts_afresh() {
    let regions = debug_regions(
        "#if DEBUG && os(macOS)\na\n#elseif os(iOS)\nb\n#endif\n#if os(macOS)\nc\n#elseif DEBUG\nd\n#endif\n",
    );
    let inside: Vec<(bool, &str)> = regions
        .into_iter()
        .filter(|(_, line)| !line.is_empty())
        .collect();
    assert_eq!(
        inside,
        [(true, "a"), (false, "b"), (false, "c"), (true, "d")]
    );
}

fn apple(files: &[(&str, &str)]) -> Repo {
    let mut all = vec![
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
        (APPLE_ENTRY, GOOD_APPLE_ENTRY),
        (APPLE_MAC, GOOD_APPLE_MAC),
        (APPLE_SESSION, GOOD_APPLE_SESSION),
    ];
    for (name, text) in files {
        all.retain(|(kept, _)| kept != name);
        all.push((name, text));
    }
    Repo::new(&all)
}

#[test]
fn an_apple_agent_held_out_of_release_passes() {
    assert_eq!(run(apple(&[]).root()), Ok(true));
}

#[test]
fn an_apple_agent_file_with_code_outside_debug_fails() {
    let mac = "#if os(macOS)\nimport AppKit\n#endif\n";
    assert_eq!(run(apple(&[(APPLE_MAC, mac)]).root()), Ok(false));
}

#[test]
fn the_apple_app_calling_the_agent_outside_debug_fails() {
    let session = "init() {\n    TestAgent.start()\n}\n";
    assert_eq!(run(apple(&[(APPLE_SESSION, session)]).root()), Ok(false));
}

#[test]
fn the_apple_switch_read_outside_the_agent_fails() {
    let elsewhere = "let socket = ProcessInfo.processInfo.environment[\"MAILCAL_TEST_AGENT\"]\n";
    let name = "clients/apple/App/AllodiaApp.swift";
    assert_eq!(run(apple(&[(name, elsewhere)]).root()), Ok(false));
}

#[test]
fn a_tree_without_the_apple_agent_is_an_error_not_a_pass() {
    let repo = Repo::new(&[
        (LINUX_MAIN, GOOD_LINUX_MAIN),
        (LINUX_AGENT, GOOD_LINUX_AGENT),
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
    ]);
    assert!(run(repo.root()).is_err());
}
