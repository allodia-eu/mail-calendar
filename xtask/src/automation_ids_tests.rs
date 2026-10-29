//! Tests for `check-automation-ids`: its readers, and its verdict over a fixture tree.

use crate::{
    automation_ids::check,
    automation_ids_scan::{
        CsLiterals, Wanted, cs_literals, cs_lookups, id_helpers, lookups, xaml_ids,
    },
    fixture::Repo,
};

fn exact(id: &str) -> Wanted {
    Wanted::Exact(id.to_owned())
}

// ---- XAML ---------------------------------------------------------------------------------------

#[test]
fn an_explicit_id_hides_the_elements_name() {
    let xaml = r#"<StackPanel>
        <TextBlock x:Name="SubjectText" Style="{StaticResource Subtitle}"
                   AutomationProperties.AutomationId="ReadingSubject" />
        <TextBlock x:Name="FromText" Text="a > b" />
    </StackPanel>"#;
    assert_eq!(xaml_ids(xaml), ["ReadingSubject", "FromText"]);
}

#[test]
fn a_bound_id_is_no_id_and_still_hides_the_name() {
    let xaml = r#"<Button x:Name="Row" AutomationProperties.AutomationId="{Binding Id}" />"#;
    assert_eq!(xaml_ids(xaml), [] as [String; 0]);
}

#[test]
fn a_commented_out_element_names_nothing() {
    let xaml = "<!-- <Button x:Name=\"Gone\" /> -->\n<Button Name=\"Here\" />";
    assert_eq!(xaml_ids(xaml), ["Here"]);
}

// ---- C# -----------------------------------------------------------------------------------------

#[test]
fn literals_interpolations_and_appended_endings_are_read() {
    let cs = r#"
        // "InAComment" is not a literal.
        var quote = '"';
        AutomationProperties.SetAutomationId(toggle, $"AccountUse_{capability}");
        AutomationProperties.SetAutomationId(box, "EventTitleBox");
        AutomationProperties.SetAutomationId(typed, automationId + "Server");
        var path = @"C:\""quoted""";
        var label = $"{count} of {(one ? "a" : "b")}";
        var script = """
            document.getElementById("NotAnId");
            """;
    "#;
    let mut out = CsLiterals::default();
    cs_literals(cs, &mut out);
    assert!(out.exact.contains(&"EventTitleBox".to_owned()));
    assert!(out.exact.contains(&"Server".to_owned()));
    assert!(
        !out.exact
            .iter()
            .any(|s| s.contains("InAComment") || s.contains("NotAnId"))
    );
    assert_eq!(
        out.prefixes,
        ["AccountUse_"],
        "a hole at the start fixes nothing"
    );
    assert_eq!(out.suffixes, ["Server"]);
}

// ---- PowerShell ---------------------------------------------------------------------------------

#[test]
fn a_helper_taking_an_id_first_is_found_and_one_taking_something_else_is_not() {
    let ps = "function Test-UiaShown {\n  param([Parameter(Mandatory)] [string] $Id)\n}\n\
              function Find-UiaElement {\n  param([string] $Name, [string] $AutomationId)\n}\n";
    assert_eq!(id_helpers(ps), ["Test-UiaShown"]);
}

#[test]
fn every_written_form_of_a_lookup_is_read() {
    let helpers = ["Test-UiaShown".to_owned()];
    let ps = r#"
$a = Find-UiaElement -AutomationId 'ReadingSubject' -Type Text
$b = $tree | Where-Object { $_.Current.AutomationId -eq "RowsList" }
$c = $tree | Where-Object { $_.Current.AutomationId -in @('DiscardButton', 'CancelButton') }
Assert-True (Test-UiaShown 'SetupUseCalendarServer') 'shown'
$d = Find-UiaElement -AutomationId "AccountUse_$use"
$e = Find-UiaElement -AutomationId $id
"#;
    let found: Vec<Wanted> = lookups(ps, &helpers).into_iter().map(|(_, w)| w).collect();
    assert_eq!(
        found,
        [
            exact("ReadingSubject"),
            exact("RowsList"),
            exact("CancelButton"),
            exact("DiscardButton"),
            exact("SetupUseCalendarServer"),
            Wanted::Prefix("AccountUse_".to_owned()),
        ]
    );
}

#[test]
fn a_comment_naming_an_id_is_not_a_lookup() {
    let ps = "# a search for -AutomationId 'SubjectText' would find the window's\n\
              <#\n  Find-UiaElement -AutomationId 'Old'\n#>\n\
              throw \"no #$Id here\" # -AutomationId 'Trailing'\n";
    assert_eq!(lookups(ps, &[]), []);
}

// ---- the whole check ----------------------------------------------------------------------------

const VIEW: &str = r#"<UserControl>
    <TextBlock x:Name="SubjectText" AutomationProperties.AutomationId="ReadingSubject" />
</UserControl>"#;

const CODE: &str =
    "class A { void B() { AutomationProperties.SetAutomationId(t, $\"AccountUse_{c}\"); } }";

fn tree(suite: &str) -> Repo {
    Repo::new(&[
        ("clients/windows/Mailcal/Views/ReadingView.xaml", VIEW),
        ("clients/windows/Mailcal/Views/ReadingView.xaml.cs", CODE),
        ("clients/windows/control.ps1", suite),
    ])
}

const PLATFORM: &[(&str, &str)] = &[("PrimaryButton", "a ContentDialog template part")];

#[test]
fn a_suite_looking_up_ids_the_app_draws_passes() {
    let repo = tree(
        "Find-UiaElement -AutomationId 'ReadingSubject'\n\
         Find-UiaElement -AutomationId 'AccountUse_Mail'\n\
         Find-UiaElement -AutomationId 'PrimaryButton'\n",
    );
    assert_eq!(check(repo.root(), PLATFORM), Ok(true));
}

#[test]
fn a_suite_still_looking_up_the_name_an_explicit_id_replaced_fails() {
    let repo = tree(
        "Find-UiaElement -AutomationId 'SubjectText'\nFind-UiaElement -AutomationId 'PrimaryButton'\n",
    );
    assert_eq!(check(repo.root(), PLATFORM), Ok(false));
}

#[test]
fn a_platform_id_no_suite_uses_any_more_fails() {
    let repo = tree("Find-UiaElement -AutomationId 'ReadingSubject'\n");
    assert_eq!(check(repo.root(), PLATFORM), Ok(false));
}

#[test]
fn a_tree_with_no_lookups_is_an_error_not_a_pass() {
    let repo = tree("Write-Host 'nothing here'\n");
    assert!(check(repo.root(), PLATFORM).is_err());
}

#[test]
fn a_csharp_suite_s_lookups_are_read_and_checked() {
    let suite = "var a = agent.Find(Filter.ById(\"ReadingSubject\", \"Text\"));
                 // Filter.ById(\"InAComment\")
                 var b = new Filter { Id = \"SubjectText\" };
";
    assert_eq!(
        cs_lookups(suite),
        [(1, exact("ReadingSubject")), (3, exact("SubjectText"))]
    );
    let repo = Repo::new(&[
        ("clients/windows/Mailcal/Views/ReadingView.xaml", VIEW),
        ("clients/windows/Mailcal/Views/ReadingView.xaml.cs", CODE),
        (
            "clients/windows/control.ps1",
            "Find-UiaElement -AutomationId 'PrimaryButton'
",
        ),
        (
            "clients/windows/Mailcal.UiTests/Suites/ReadingTests.cs",
            suite,
        ),
    ]);
    assert_eq!(
        check(repo.root(), PLATFORM),
        Ok(false),
        "SubjectText is the name the explicit id hides"
    );
}
