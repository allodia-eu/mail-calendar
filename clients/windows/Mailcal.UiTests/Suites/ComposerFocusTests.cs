// Where the composer's caret opens, and that the suggestion list floats rather than displaces
// (docs/contacts.md §4).
//
// WHY HERE. FocusesBody, the predicate the contract binds every client to, lives on ComposeRequest,
// which builds its title from L10n and cannot be linked into Mailcal.Tests; and the predicate
// answering correctly proves nothing about whether anything acted on it.
//
// WHY THE HARNESS. The list has to be long enough to reach the editor, §4 says so, because a list
// that stops above the web view passes while the bug is still there. The showcase indexes two
// people, a list shorter than the gap below every recipient field; the harness seeds ten-odd
// addresses, which fill the list to its cap and put part of it over the editor.
//
// Nothing here sends: composers are opened, read and discarded.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class ComposerFocusTests(HarnessApp harness)
{
    // How many suggestions the overlap assertion needs before it means anything: three already clear
    // the gap between the field and the editor at 200% and two do not, so four is that with a margin.
    // Below it, "the list does not reach the editor" is the fixture being thin, not the rule broken.
    private const int MinMatchesForOverlap = 4;

    // A token most of the seeded address book matches: every seeded address carries an 'e'.
    private const string BroadToken = "e";

    private void Case(string name, Action<App, DraftsFlow> body) =>
        harness.Case(name, app => body(app, new DraftsFlow(app)), app => app.CloseComposer());

    private static Element Field(App app, string id) =>
        app.Agent.Find(Filter.ById(id) with { OnScreen = true }) ?? throw new InvalidOperationException($"the composer draws no #{id}");

    private static bool Focused(App app, string id) => app.Agent.Find(Filter.ById(id)) is { } element && app.Agent.Read(element).Focused;

    /// <summary>The caret is placed after the editor's seed, which waits on the WebView2, so the delay
    /// is real and varies by machine.</summary>
    private static bool WaitFocused(App app, string id) => App.Eventually(() => Focused(app, id), TimeSpan.FromSeconds(25));

    /// <summary>Types <paramref name="token"/> into To as a person does, the field only offering
    /// suggestions while it has the keyboard, and returns the list it opens.</summary>
    private static Element Suggestions(App app, string token)
    {
        app.TypeInto(Field(app, "ToField"), token);
        return app.Agent.Wait(Filter.ById("RecipientSuggestions") with { OnScreen = true }, TimeSpan.FromSeconds(15))
            ?? throw new InvalidOperationException($"typing '{token}' into To offered no suggestions at all, so this case measured nothing");
    }

    private static Dictionary<string, double> Tops(App app) =>
        new[] { "SubjectBox", "Editor", "SendButton" }.ToDictionary(id => id, id => Field(app, id).Y);

    [HarnessFact]
    public void ANewMessageOpensWithTheCaretInTo() => Case(nameof(ANewMessageOpensWithTheCaretInTo), (app, flow) =>
    {
        flow.OpenNewComposer();
        Assert.True(WaitFocused(app, "ToField"),
            "the composer opened with To unfocused: writing a new message costs a click before a character can be typed (docs/contacts.md §4)");
    });

    [HarnessFact]
    public void AReplyOpensWithTheCaretInTheBodyNotInTo() => Case(nameof(AReplyOpensWithTheCaretInTheBodyNotInTo), (app, flow) =>
    {
        var row = app.Rows().FirstOrDefault() ?? throw new InvalidOperationException("the message list is empty, so there is nothing to reply to");
        app.Agent.Invoke(row);
        app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingReply") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the reading pane never offered Reply"));
        if (app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of pressing Reply");
        }
        flow.WaitEditorSeeded();
        Assert.True(WaitFocused(app, "Editor"),
            "a reply opened without the caret in the body; its From, To and Subject are filled in, so writing is all that is left (docs/contacts.md §4)");
        // The other half of "exactly one of the two": a client that focused both in turn would pass
        // the assertion above.
        Assert.False(Focused(app, "ToField"), "a reply put the caret in To as well as the body");
    });

    [HarnessFact]
    public void TheSuggestionListFloatsNothingBelowItMoves() => Case(nameof(TheSuggestionListFloatsNothingBelowItMoves), (app, flow) =>
    {
        flow.OpenNewComposer();
        var before = Tops(app);
        Suggestions(app, BroadToken);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var after = Tops(app);
        foreach (var (id, top) in before)
        {
            Assert.True(top == after[id],
                $"#{id} moved from {top} to {after[id]} when the suggestion list opened: the list takes layout space, so the form jumps on every keystroke of the first recipient (docs/contacts.md §4)");
        }
    });

    [HarnessFact]
    public void TheListIsLongEnoughToReachTheEditorAndIsDrawnOverIt() => Case(nameof(TheListIsLongEnoughToReachTheEditorAndIsDrawnOverIt), (app, flow) =>
    {
        flow.OpenNewComposer();
        var list = Suggestions(app, BroadToken);
        // Before the geometry, and separately: a short list clears the editor honestly, and would
        // report the fixture thinning out as the rule holding.
        var matches = app.Agent.FindAll(new Filter { Type = "ListItem", Under = list }).Count;
        Assert.True(matches >= MinMatchesForOverlap,
            $"only {matches} suggestion(s) matched '{BroadToken}', too short a list to reach the editor, so this case can prove nothing. Reseed the harness (scripts/dev/harness.sh up)");
        list = app.Steady(list);
        var editorTop = Field(app, "Editor").Y;
        Assert.True(list.Y + list.Height > editorTop,
            "the suggestion list stops above the message editor, so this run never tested whether it can cover a hosted web view, the half of §4 that does not come free");
    });

    [HarnessFact]
    public void OnlyTheFocusedFieldOffersSuggestions() => Case(nameof(OnlyTheFocusedFieldOffersSuggestions), (app, flow) =>
    {
        flow.OpenNewComposer();
        app.Agent.Toggle(Field(app, "CcBccToggle"));
        var cc = app.Agent.Wait(Filter.ById("CcField") with { OnScreen = true }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Cc never appeared");
        Suggestions(app, BroadToken);
        app.Agent.Focus(cc);
        // The field closes its list on LosingFocus, raised on the UI thread.
        Assert.True(app.Agent.WaitGone(Filter.ById("RecipientSuggestions") with { OnScreen = true }, TimeSpan.FromSeconds(5)),
            "To's suggestion list is still on screen with the caret in Cc: floating, it covers a live field (docs/contacts.md §4)");
    });
}
