// Enter and leaving the field finish an address as a comma does (docs/contacts.md), and neither a
// pill's remove button nor the suggestion list is leaving the field. The keys are real and the pill
// is clicked, never invoked: whether focus leaves the input is the whole question, and an invoke
// moves no focus. Nothing is sent; each composer is discarded.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class RecipientFinishTests(ShowcaseApp showcase)
{
    private const string Address = "bob@example.test";

    private void Case(string name, Action<App, Element> body) =>
        showcase.Case(name, app => body(app, OpenComposer(app)), app => app.CloseComposer());

    /// <summary>A new composer, its caret in To, ready for keys: typed before the editor is seeded,
    /// keys are wiped with it.</summary>
    private static Element OpenComposer(App app)
    {
        new DraftsFlow(app).OpenNewComposer();
        var to = app.Agent.Find(Filter.ById("ToField") with { OnScreen = true }) ?? throw new InvalidOperationException("no ToField on screen");
        app.ClickToType(to);
        return to;
    }

    private static void Keys(App app, ushort key, params ushort[] modifiers)
    {
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.Chord(key, modifiers);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
    }

    private static void Type(App app, string text)
    {
        Input.Type(text);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
    }

    private static List<Element> Pills(App app) =>
        app.Agent.FindAll(new Filter { Type = "Button", NameLike = "*@example.test*" }).ToList();

    private static string Value(App app, Element to) => app.Agent.Read(to).Value ?? "";

    [ShowcaseFact]
    public void EnterFinishesTheTypedAddressAndKeepsTheCaretInTheField() => Case(nameof(EnterFinishesTheTypedAddressAndKeepsTheCaretInTheField), (app, to) =>
    {
        Type(app, Address);
        Keys(app, Input.Enter);
        Assert.True(Pills(app).Count == 1, "Enter finishes the typed address into a pill");
        Assert.Equal("", Value(app, to));
        Assert.True(app.Agent.Read(to).Focused, "Enter keeps the caret in To for the next address");
        Keys(app, 'Z', Input.Control);
        Assert.Equal(Address, Value(app, to));
    });

    [ShowcaseFact]
    public void LeavingTheFieldFinishesTheTypedAddress() => Case(nameof(LeavingTheFieldFinishesTheTypedAddress), (app, _) =>
    {
        Type(app, Address);
        Keys(app, Input.Tab);
        Assert.True(Pills(app).Count == 1, "leaving To finishes the typed address into a pill");
    });

    [ShowcaseFact]
    public void LeavingTheFieldByShiftTabThroughAPillFinishesTheTypedAddress() => Case(nameof(LeavingTheFieldByShiftTabThroughAPillFinishesTheTypedAddress), (app, to) =>
    {
        Type(app, $"{Address},carol@example.test");
        Keys(app, Input.Tab, Input.Shift);
        Assert.True(Value(app, to) == "carol@example.test", "a pill's remove button is still the field, so focus on it finishes nothing");
        Assert.True(Pills(app).Count == 1, "focus on a pill finishes nothing");
        Keys(app, Input.Tab, Input.Shift);
        var focused = app.Agent.Focused();
        Assert.True(Pills(app).Count == 2, $"leaving the field from a pill finishes the typed address (focus is on {focused?.Type} '{focused?.Name}')");
    });

    [ShowcaseFact]
    public void AClickOnAPillsRemoveButtonRemovesItAndFinishesNothing() => Case(nameof(AClickOnAPillsRemoveButtonRemovesItAndFinishesNothing), (app, to) =>
    {
        Type(app, $"{Address},carol");
        Assert.True(Pills(app).Count == 1, "a comma finishes an address into a pill");
        Input.Click(app.Steady(Pills(app)[0]));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
        Assert.True(Pills(app).Count == 0, "the click reaches the remove button");
        Assert.Equal("carol", Value(app, to));
    });
}
