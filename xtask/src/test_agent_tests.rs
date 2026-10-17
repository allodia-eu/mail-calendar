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
    let repo = Repo::new(&[(ENTRY, GOOD_ENTRY), (TREE, GOOD_TREE)]);
    assert_eq!(run(repo.root()), Ok(true));
}

#[test]
fn an_entry_point_a_release_build_would_keep_fails() {
    let entry = GOOD_ENTRY.replace("    [Conditional(\"DEBUG\")]\n", "");
    let repo = Repo::new(&[(ENTRY, entry.as_str()), (TREE, GOOD_TREE)]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn an_agent_file_with_code_outside_debug_fails() {
    let tree = GOOD_TREE.replace("#if DEBUG\n", "").replace("#endif\n", "");
    let repo = Repo::new(&[(ENTRY, GOOD_ENTRY), (TREE, tree.as_str())]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn the_switch_read_outside_debug_anywhere_in_the_app_fails() {
    let elsewhere = "namespace X;\nclass App { string s = System.Environment.GetEnvironmentVariable(\"MAILCAL_TEST_AGENT\"); }\n";
    let repo = Repo::new(&[
        (ENTRY, GOOD_ENTRY),
        (TREE, GOOD_TREE),
        ("clients/windows/Mailcal/App.xaml.cs", elsewhere),
    ]);
    assert_eq!(run(repo.root()), Ok(false));
}

#[test]
fn a_tree_without_the_agent_is_an_error_not_a_pass() {
    let repo = Repo::new(&[(TREE, GOOD_TREE)]);
    assert!(run(repo.root()).is_err());
}
