// The composer opens on the header a message usually needs, From, To, Subject, with Cc and Bcc
// behind the chevron on the To row (docs/composer-security.md, Gate 12). Mailcal.Tests reaches the
// rule that decides whether the row opens revealed (RecipientTokens.RevealsCcBcc) and nothing else:
// a composer that opened with the row showing, or a chevron wired to nothing, passes it.
//
// The fields are reached by the id on their input, not their label, which is localised; the three
// are one control used three times. A collapsed field is in no tree, which is what makes "Cc is not
// on screen" something this can fail on. Nothing is sent; each composer is discarded.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class ComposerCcBccTests(ShowcaseApp showcase)
{
    // How far the chevron's centre may sit from the To input's, in the units XAML is written in.
    // Small on purpose: the defect drew it 4 epx low, aligned against the whole field rather than
    // its input, whose closed suggestion popup sits under the input.
    private const double CentreToleranceDip = 2;

    // The least height the editor keeps once Cc and Bcc are open on a short window. Before the
    // header scrolled, a 640-epx window left it about 20: a sliver nobody can write in.
    private const double EditorFloorDip = 120;

    private void Case(string name, Action<App> body) =>
        showcase.Case(name, app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            body(app);
        }, app => app.CloseComposer());

    private static Element? Field(App app, string id) => app.Agent.Find(Filter.ById(id));

    private static Element Chevron(App app) =>
        Field(app, "CcBccToggle") ?? throw new InvalidOperationException("no CcBccToggle: the composer draws no way to reach Cc and Bcc at all");

    [OfflineFact]
    public void ANewMessageOpensWithCcAndBccCollapsed() => Case(nameof(ANewMessageOpensWithCcAndBccCollapsed), app =>
    {
        Assert.True(Field(app, "ToField") is not null, "the composer opened without To, the one recipient field it always shows");
        Assert.True(Field(app, "CcField") is null, "Cc is on screen before anyone asked for it; the header opens as From, To, Subject");
        Assert.True(Field(app, "BccField") is null, "Bcc is on screen before anyone asked for it");
    });

    [OfflineFact]
    public void TheChevronIsDrawnLevelWithTheToInputNotWithTheFieldAroundIt() => Case(nameof(TheChevronIsDrawnLevelWithTheToInputNotWithTheFieldAroundIt), app =>
    {
        var input = app.Steady(Field(app, "ToField")!);
        var chevron = app.Steady(Chevron(app));
        var drift = Math.Abs((chevron.Y + (chevron.Height / 2)) - (input.Y + (input.Height / 2)));
        var budget = CentreToleranceDip * Input.Scale(input.Window);
        Assert.True(drift <= budget, $"the chevron's centre is {drift}px off the To input's, against {budget}px: it is aligned against something other than its input");
    });

    [OfflineFact]
    public void TheChevronRevealsCcAndBccAndPutsThemAwayAgain() => Case(nameof(TheChevronRevealsCcAndBccAndPutsThemAwayAgain), app =>
    {
        app.Agent.SetToggle(Chevron(app), on: true);
        Assert.True(app.Agent.Wait(Filter.ById("CcField"), TimeSpan.FromSeconds(3)) is not null, "the chevron is checked and Cc is not on screen: the toggle is wired to nothing");
        Assert.True(Field(app, "BccField") is not null, "the chevron is checked and Bcc is not on screen");
        app.Agent.SetToggle(Chevron(app), on: false);
        Assert.True(app.Agent.WaitGone(Filter.ById("CcField"), TimeSpan.FromSeconds(3)), "Cc stayed on screen after the chevron was unchecked: the reveal is one-way");
    });

    [OfflineFact]
    public void OnAShortWindowTheMessageKeepsRoomToWriteInWithCcAndBccOpen()
    {
        IDisposable? shrunk = null;
        showcase.Case(nameof(OnAShortWindowTheMessageKeepsRoomToWriteInWithCcAndBccOpen), app =>
        {
            shrunk = Desktop.Resized(app.Mailbox.Hwnd, 1100, 640);
            new DraftsFlow(app).OpenNewComposer();
            app.Agent.SetToggle(Chevron(app), on: true);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
            var editor = app.Steady(Field(app, "Editor") ?? throw new InvalidOperationException("no editor"));
            var floor = EditorFloorDip * Input.Scale(editor.Window);
            Assert.True(editor.Height >= floor, $"the editor is {editor.Height}px tall against a {floor}px floor: the header took the height the message needs instead of scrolling");
            Assert.True(Field(app, "SendButton") is { Rendered: true }, "Send is gone from the composer on a short window");
        }, app =>
        {
            app.CloseComposer();
            shrunk?.Dispose();
        });
    }
}
