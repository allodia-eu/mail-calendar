// The message list draws nothing under the pointer. A mouse moving down a mailbox crosses every row
// on its way to the one it wants, and a label following it covers the rows being scanned.
//
// THE REGRESSION. The list carries keyboard accelerators for Delete, Back and Escape
// (docs/list-selection.md, rule 9), and KeyboardAcceleratorPlacementMode defaults to Auto, which has
// the framework raise a tooltip naming the key over whatever the pointer rests on. It is the
// framework's word, not ours, so no catalog check sees it, and it is in no markup: the only way to
// know it is there is a real pointer on a row in a running window. The first case proves the hover
// is real before the second leans on it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class MailListHoverTests(ShowcaseApp showcase)
{
    // The tooltip's own delay, with room to spare: less would assert only that nothing had appeared yet.
    private const int HoverSettleMs = 1800;

    private static List<string> ToolTips(App app) =>
        app.Agent.FindAll(new Filter { Type = "ToolTip" }).Select(t => t.Name).ToList();

    /// <summary>A row well down the list, so the pointer has rows to cross on its way.</summary>
    private static Element TargetRow(App app)
    {
        var rows = app.Rows();
        return rows.Count >= 4 ? app.Steady(rows[3]) : throw new InvalidOperationException($"the list holds {rows.Count} row(s); this needs a few to cross");
    }

    [ShowcaseFact]
    public void TheHoverIsRealAButtonThatHasATooltipRaisesIt() => showcase.Case(nameof(TheHoverIsRealAButtonThatHasATooltipRaisesIt), app =>
    {
        // The control for the case below, which asserts that nothing appears and so passes as readily
        // against a pointer that never arrived. Reply, because it carries a tooltip and is enabled
        // whenever a message is open: a disabled button raises none.
        // Pressing the row already open would close it.
        if (app.Agent.Find(Filter.ById("ReadingReply", "Button") with { OnScreen = true }) is null)
        {
            app.Agent.Invoke(app.Rows().FirstOrDefault() ?? throw new InvalidOperationException("the list is empty"));
        }
        var reply = app.Agent.Wait(Filter.ById("ReadingReply", "Button") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the reading pane drew no Reply");
        Input.Hover(app.Steady(reply), HoverSettleMs);
        Assert.True(ToolTips(app).Count == 1, "resting the pointer on Reply raised no tooltip, so the pointer does not reach the window and the next case would pass whatever the list draws");
    });

    [ShowcaseFact]
    public void HoveringAMessageRowSaysNothing() => showcase.Case(nameof(HoveringAMessageRowSaysNothing), app =>
    {
        Input.Hover(TargetRow(app), HoverSettleMs);
        var tips = ToolTips(app);
        Assert.True(tips.Count == 0,
            $"the pointer resting on a message row raised: {string.Join(" | ", tips)}. The framework raises one of its own for a keyboard accelerator unless the list says not to");
    });

    [ShowcaseFact]
    public void AndTheKeysTheListIsSilentAboutStillWork() => showcase.Case(nameof(AndTheKeysTheListIsSilentAboutStillWork), app =>
    {
        // Silencing the accelerator's tooltip must not take the accelerator with it. Escape, because
        // clearing a selection costs no message; the row is selected through its pattern and given
        // the keyboard, since a click would also open it in the pane.
        var row = TargetRow(app);
        app.Agent.Select(row);
        app.Agent.Focus(row);
        bool Selected() => app.Agent.Read(row).Selected == true;
        Assert.True(App.Eventually(Selected, TimeSpan.FromSeconds(3)), "a row is selected to clear");
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.Press(Input.Escape);
        Assert.True(App.Eventually(() => !Selected(), TimeSpan.FromSeconds(3)),
            "Escape still clears the selection: the placement mode decides whether the framework announces an accelerator, never whether it fires");
    });
}
