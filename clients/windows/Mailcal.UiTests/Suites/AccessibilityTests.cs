// What a screen reader announces: the one property of the UI no other gate sees and nothing about
// looking at the app reveals. Both rules were red when this was written, on a build whose every
// other gate was green: every mailbox row announced "Allodia.Mailcal.ViewModels.MailRow", a
// templated ListViewItem falling back to its object's type name, and six buttons announced nothing,
// their content an icon and label panel from which WinUI derives no name. Neither shows in a
// screenshot, and neither reaches Mailcal.Tests, which links no WinUI.
//
// The agent reads what UI Automation's control view does, an element hidden from assistive
// technology left out (README). The showcase, pinned to English, so the state words are stable. The
// cases run in the order written: the last goes to the calendar.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed partial class AccessibilityTests(ShowcaseApp showcase)
{
    // A .NET type rather than a phrase: two or more dotted segments and no spaces. A shape rather than
    // "MailRow", because the fallback announces whatever class next arrives in a list.
    [GeneratedRegex(@"^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*){2,}$")]
    private static partial Regex TypeName();

    // What a screen-reader user navigates to and acts on; a text or image may be decorative.
    private static readonly HashSet<string> Interactive = ["Button", "ListItem", "CheckBox", "ComboBox", "TabItem", "MenuItem", "Hyperlink"];

    /// <summary>The texts drawn inside <paramref name="row"/>, which its announcement has to carry.</summary>
    private static List<string> TextsIn(App app, Element row) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = row }).Select(t => t.Name).Where(n => n.Length > 0).ToList();

    /// <summary>Each row announces every text it shows, a conversation's count badge aside, which is
    /// chrome rather than identity and a bare numeral that matches anything.</summary>
    private static void AssertAnnouncesWhatItShows(App app, IReadOnlyList<Element> rows, string what)
    {
        Assert.NotEmpty(rows);
        foreach (var row in rows)
        {
            Assert.DoesNotMatch(TypeName(), row.Name);
            var texts = TextsIn(app, row);
            Assert.True(texts.Count > 0, $"a seeded {what} draws at least a title");
            foreach (var text in texts.Where(t => !t.All(char.IsDigit)))
            {
                Assert.True(row.Name.Contains(text, StringComparison.Ordinal),
                    $"the {what} shows '{text}' and announces '{row.Name}': a screen reader hears what the row says, or the two describe different things");
            }
        }
    }

    [OfflineFact]
    public void NoInteractiveElementAnnouncesADotNetTypeName() => showcase.Case(nameof(NoInteractiveElementAnnouncesADotNetTypeName), app =>
    {
        var offenders = app.Agent.FindAll(new Filter { OnScreen = true }).Where(e => Interactive.Contains(e.Type) && TypeName().IsMatch(e.Name)).Select(e => e.Name).Distinct().ToList();
        Assert.True(offenders.Count == 0, $"a class name is what a screen reader reads aloud, so these are unusable without sight and identical to each other: {string.Join(", ", offenders)}");
    });

    [OfflineFact]
    public void EveryMailboxRowAnnouncesItsSenderSubjectAndDate() => showcase.Case(nameof(EveryMailboxRowAnnouncesItsSenderSubjectAndDate), app =>
        AssertAnnouncesWhatItShows(app, app.Rows(), "row"));

    [OfflineFact]
    public void TheAvatarBesideASenderIsNotAnnounced() => showcase.Case(nameof(TheAvatarBesideASenderIsNotAnnounced), app =>
    {
        // docs/avatars.md: the circle restates the sender's initials, so announced, a screen reader
        // reads letters before every message. Only its Raw view keeps the text inside quiet. A
        // monogram is one or two capitals, which no seeded subject, sender or date is.
        var rows = app.Rows();
        Assert.NotEmpty(rows);
        var spoken = rows.SelectMany(row => TextsIn(app, row)).Where(t => Regex.IsMatch(t, "^[A-Z]{1,2}$")).ToList();
        Assert.True(spoken.Count == 0, $"a monogram reaches the accessibility tree, read before every sender's name: {string.Join(", ", spoken)}");
    });

    [OfflineFact]
    public void EveryButtonCarriesASpokenName() => showcase.Case(nameof(EveryButtonCarriesASpokenName), app =>
    {
        // A WinUI button derives a name from string content, never from an icon and label panel. On
        // screen: a view never shown, such as the welcome screen behind the showcase's mailbox, has
        // not bound its content yet.
        var unnamed = app.Agent.FindAll(new Filter { Type = "Button", OnScreen = true }).Where(b => b.Name.Length == 0)
            .Select(b => $"#{b.Id} ({b.X:0},{b.Y:0}) {b.Width:0}x{b.Height:0}").ToList();
        Assert.True(unnamed.Count == 0, $"an unnamed button announces only \"button\": {string.Join(" ", unnamed)}");
    });

    [OfflineFact]
    public void EverySettingsPickerCarriesASpokenName() => showcase.Case(nameof(EverySettingsPickerCarriesASpokenName), app =>
    {
        // A combo box takes no name from a heading beside it, so without one it announces only its
        // value: what the setting is set to, never what it is. Per category, because the panel
        // builds only the open one.
        var settings = new SetupFlow(app);
        var categories = settings.Categories();
        Assert.NotEmpty(categories);
        var unnamed = categories
            .SelectMany(tag => app.Agent.FindAll(new Filter { Type = "ComboBox", Under = settings.OpenCategory(tag) }).Where(c => c.Name.Length == 0).Select(_ => tag))
            .ToList();
        Assert.True(unnamed.Count == 0, $"a picker with no name says what a setting is set to and never what it is; give it AutomationProperties.Name or a Header. Unnamed under: {string.Join(", ", unnamed)}");
    }, app => app.CloseDialogs());

    [OfflineFact]
    public void EveryAgendaRowAnnouncesItsEventNotItsClass() => showcase.Case(nameof(EveryAgendaRowAnnouncesItsEventNotItsClass), app =>
    {
        // The sweep above sees only what is on screen, which is how the agenda kept the template
        // fallback after the mailbox lost it. The agenda rather than the grid, whose events have no
        // text of their own.
        var agenda = new CalendarFlow(app).ShowAgenda();
        app.Agent.Wait(new Filter { Type = "ListItem", Under = agenda }, App.Patience);
        AssertAnnouncesWhatItShows(app, app.Agent.FindAll(new Filter { Type = "ListItem", Under = agenda }), "agenda row");
    });
}
