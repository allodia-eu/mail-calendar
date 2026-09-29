// A drafted reply's card (docs/ai.md, "Summary and checklist" and "Where they show"): the items in
// the core's order, a fill-in item that follows the reply and nothing else, the others ticked by the
// person, an answer about an earlier draft dropped, and Send asking once per composer while anything
// is open.

using System.Linq;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class DraftChecklistTests
{
    private static readonly DraftTask[] Tasks =
    [
        new(DraftTaskKind.FillIn, "[date]"),
        new(DraftTaskKind.FillIn, "[time]"),
        new(DraftTaskKind.Attach, "Attach the agenda"),
        new(DraftTaskKind.Do, "Book the room"),
    ];

    private static DraftChecklist Drafted(DraftTask[]? tasks = null, int attachments = 0)
    {
        var checklist = new DraftChecklist();
        checklist.Show("Bob asks when you can meet.", tasks ?? Tasks, attachments);
        return checklist;
    }

    private static bool[] Ticks(DraftChecklist checklist) => checklist.Items.Select(item => item.Ticked).ToArray();

    [Fact]
    public void ItemsKeepTheCoresOrderAndALaterDraftReplacesThem()
    {
        var checklist = Drafted();
        Assert.Equal(
            new[] { "[date]", "[time]", "Attach the agenda", "Book the room" },
            checklist.Items.Select(item => item.Text).ToArray());
        checklist.Toggle(3);
        var first = checklist.Draft;
        checklist.Show("", [new DraftTask(DraftTaskKind.Do, "Call Bob")], 0);
        Assert.Equal(new[] { "Call Bob" }, checklist.Items.Select(item => item.Text).ToArray());
        Assert.Equal(1, checklist.OpenCount);
        Assert.Empty(checklist.Summary);
        Assert.NotEqual(first, checklist.Draft);
    }

    [Fact]
    public void ADraftWithNothingToSayShowsNoCard()
    {
        Assert.True(new DraftChecklist().IsEmpty);
        var checklist = new DraftChecklist();
        checklist.Show("", [], 0);
        Assert.True(checklist.IsEmpty);
        checklist.Show("  ", [], 0);
        Assert.True(checklist.IsEmpty);
        checklist.Show("Bob asks for the slides.", [], 0);
        Assert.False(checklist.IsEmpty);
    }

    // Ticked exactly while the placeholder is gone, so an undo opens the item again.
    [Fact]
    public void AFillInItemFollowsWhatIsLeftInTheReply()
    {
        var checklist = Drafted();
        Assert.Equal(new[] { "[date]", "[time]" }, checklist.Placeholders);
        Assert.True(checklist.AwaitsPlaceholders);
        Assert.True(checklist.PlaceholdersLeft(["[time]"], checklist.Draft));
        Assert.Equal(new[] { true, false, false, false }, Ticks(checklist));
        Assert.True(checklist.AwaitsPlaceholders);
        checklist.PlaceholdersLeft([], checklist.Draft);
        Assert.False(checklist.AwaitsPlaceholders);
        checklist.PlaceholdersLeft(["[date]"], checklist.Draft);
        Assert.Equal(new[] { false, true, false, false }, Ticks(checklist));
        Assert.True(checklist.AwaitsPlaceholders);
    }

    // The editor is asked about one draft and may answer after the next went in; that answer is
    // about placeholders the card no longer holds.
    [Fact]
    public void AnAnswerAboutAnEarlierDraftTicksNothing()
    {
        var checklist = Drafted();
        var earlier = checklist.Draft;
        checklist.Show("", Tasks, 0);
        Assert.False(checklist.PlaceholdersLeft([], earlier));
        Assert.Equal(new[] { false, false, false, false }, Ticks(checklist));
    }

    [Fact]
    public void OnlyTheItemsTheEditorCannotSeeAreTickedByHand()
    {
        var checklist = Drafted();
        checklist.Toggle(0);
        Assert.False(checklist.Items[0].Ticked);
        checklist.Toggle(2);
        checklist.Toggle(3);
        Assert.True(checklist.Items[2].Ticked && checklist.Items[3].Ticked);
        checklist.Toggle(3);
        Assert.False(checklist.Items[3].Ticked);
        checklist.PlaceholdersLeft([], checklist.Draft);
        Assert.True(checklist.Items[2].Ticked, "the reply leaves an attach item alone");
    }

    // Which file answers which item cannot be told, so an attach item ticks only once there is a
    // new file for every one of them.
    [Fact]
    public void AttachItemsTickOnceEachHasANewFile()
    {
        DraftTask[] tasks =
        [
            new(DraftTaskKind.Attach, "Attach the agenda"),
            new(DraftTaskKind.Attach, "Attach the minutes"),
            new(DraftTaskKind.Do, "Book the room"),
        ];
        var checklist = Drafted(tasks, attachments: 1);
        checklist.AttachmentsChanged(2);
        Assert.Equal(new[] { false, false, false }, Ticks(checklist));
        checklist.AttachmentsChanged(3);
        Assert.Equal(new[] { true, true, false }, Ticks(checklist));

        var none = Drafted([new DraftTask(DraftTaskKind.Do, "Book the room")]);
        none.AttachmentsChanged(4);
        Assert.False(none.Items[0].Ticked);
    }

    [Fact]
    public void TheOpenCountIsEveryUntickedItem()
    {
        var checklist = Drafted();
        Assert.Equal(4, checklist.OpenCount);
        checklist.PlaceholdersLeft(["[time]"], checklist.Draft);
        checklist.Toggle(2);
        Assert.Equal(2, checklist.OpenCount);
    }

    // Either answer ends the asking for the composer, a later draft included; and with nothing
    // open it never asks at all.
    [Fact]
    public void SendAsksOnceAndOnlyWhileSomethingIsOpen()
    {
        var checklist = Drafted();
        Assert.True(checklist.AsksBeforeSend);
        checklist.SendAsked();
        Assert.False(checklist.AsksBeforeSend);
        checklist.Show("", Tasks, 0);
        Assert.True(checklist.HasAsked);
        Assert.False(checklist.AsksBeforeSend);

        var done = Drafted([new DraftTask(DraftTaskKind.Do, "Book the room")]);
        done.Toggle(0);
        Assert.False(done.AsksBeforeSend);
        Assert.False(new DraftChecklist().AsksBeforeSend);
    }

    // WebView2 hands the answer back JSON-encoded, and a hook that failed answers "null": no list
    // is no answer, so nothing is ticked on it.
    [Fact]
    public void TheEditorsAnswerIsAListOrNothing()
    {
        Assert.Equal(new[] { "[time]" }, DraftChecklist.ReadLeft("[\"[time]\"]"));
        Assert.Equal(new[] { "[a]" }, DraftChecklist.ReadLeft("[1, \"[a]\", null]"));
        Assert.Empty(DraftChecklist.ReadLeft("[]")!);
        Assert.Null(DraftChecklist.ReadLeft("null"));
        Assert.Null(DraftChecklist.ReadLeft("{}"));
        Assert.Null(DraftChecklist.ReadLeft("not json"));
        Assert.Null(DraftChecklist.ReadLeft(null));
    }

    // The placeholders are the reply's own words; they go in as JSON, so none can close the call.
    [Fact]
    public void ThePlaceholdersGoToTheEditorAsJson()
    {
        var script = DraftChecklist.LeftScript(["[date]", "[a \"quote\"]"]);
        Assert.StartsWith("window.composerPlaceholdersLeft([", script);
        Assert.EndsWith("])", script);
        var list = script["window.composerPlaceholdersLeft(".Length..^1];
        Assert.Equal(new[] { "[date]", "[a \"quote\"]" }, DraftChecklist.ReadLeft(list));
    }

    // In a short window the card would otherwise take the height the reply needs: the editor keeps
    // its floor while the card's body can give way, and a tall window caps the body as before.
    [Fact]
    public void TheCardYieldsToTheEditorInAShortWindow()
    {
        // A 760-DIP window on a 1440 by 900 display leaves the editor and the body 169 between them.
        Assert.Equal(DraftChecklist.CardBodyFloor, DraftChecklist.CardBodyCeiling(169));
        Assert.Equal(100, DraftChecklist.CardBodyCeiling(DraftChecklist.EditorFloor + 100));
        Assert.Equal(DraftChecklist.CardBodyMax, DraftChecklist.CardBodyCeiling(900));
        Assert.Equal(DraftChecklist.CardBodyFloor, DraftChecklist.CardBodyCeiling(0));
    }

    // Where both floors do not fit, an open card would leave the reply a line or two, so it opens
    // folded to its heading; where they do, it opens.
    [Fact]
    public void TheCardOpensFoldedWhereBothFloorsDoNotFit()
    {
        const double both = DraftChecklist.EditorFloor + DraftChecklist.CardBodyFloor;
        Assert.True(DraftChecklist.CardOpensFolded(167));
        Assert.True(DraftChecklist.CardOpensFolded(both - 1));
        Assert.False(DraftChecklist.CardOpensFolded(both));
        Assert.False(DraftChecklist.CardOpensFolded(600));
    }

    // The editor takes whatever the body leaves, so their sum is the same under any cap, and the
    // cap worked out after the layout has taken it is the cap that was applied.
    [Theory]
    [InlineData(169)]
    [InlineData(300)]
    [InlineData(420)]
    public void TheCapDoesNotMoveOnceApplied(double editorAndBody)
    {
        var cap = DraftChecklist.CardBodyCeiling(editorAndBody);
        var editor = editorAndBody - cap;
        Assert.Equal(cap, DraftChecklist.CardBodyCeiling(editor + cap));
    }
}
