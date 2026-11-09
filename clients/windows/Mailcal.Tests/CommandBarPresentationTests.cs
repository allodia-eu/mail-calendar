// How the Windows command bar reads the core's answer (docs/command-bar.md), without a window. Each
// of these fails silently in the running app: a toggle whose slot changes its id when it changes its
// half loses its place to the overflow's measuring and to every test that finds it; a pick that
// does not carry the core's target or confirm runs a different action from the row menu's; and a
// Delete key that looks for Delete alone does nothing in Trash.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CommandBarPresentationTests
{
    private static readonly MenuMessage Newest = new("acc-1", "m-3", MessageKind.Received);

    private static CommandItem Act(RowAction action, bool enabled = true, MenuMessage? target = null, bool confirm = false) =>
        new(new Command.Act(action), enabled, target, confirm);

    private static CommandBar Bar(RowAction delete) => new(
        [
            new CommandSection([Act(delete), Act(RowAction.Archive), Act(RowAction.MoveToFolder), Act(RowAction.Flag), Act(RowAction.MarkRead)]),
            new CommandSection([new CommandItem(new Command.Sync(), true, null, false)]),
        ],
        [Act(RowAction.MarkAsSpam, enabled: false)]);

    /// <summary>A slot is one place on the bar whichever half of its toggle the core offers, and
    /// Delete's slot is Delete permanently's in Trash.</summary>
    [Fact]
    public void EachSlotKeepsItsIdWhicheverHalfItOffers()
    {
        Assert.Equal("CommandDelete", CommandBarPresentation.AutomationId(new Command.Act(RowAction.Delete)));
        Assert.Equal("CommandDelete", CommandBarPresentation.AutomationId(new Command.Act(RowAction.PermanentlyDelete)));
        Assert.Equal("CommandFlag", CommandBarPresentation.AutomationId(new Command.Act(RowAction.Unflag)));
        Assert.Equal("CommandRead", CommandBarPresentation.AutomationId(new Command.Act(RowAction.MarkUnread)));
        Assert.Equal("CommandReport", CommandBarPresentation.AutomationId(new Command.Act(RowAction.MarkAsNotSpam)));
        Assert.Equal("CommandSync", CommandBarPresentation.AutomationId(new Command.Sync()));
        var ids = Bar(RowAction.Delete).Sections.SelectMany(s => s.Items).Concat(Bar(RowAction.Delete).Menu)
            .Select(item => CommandBarPresentation.AutomationId(item.Command)).ToList();
        Assert.Equal(ids.Count, ids.Distinct().Count());
    }

    /// <summary>Rule 1: a pick runs as the same item in a row menu does, so the menu item it becomes
    /// carries the core's target and its question.</summary>
    [Fact]
    public void APickIsTheRowMenuItemWithTheCoresTargetAndQuestion()
    {
        var flag = CommandBarPresentation.MenuItem(Act(RowAction.Flag, target: Newest));
        Assert.Equal(new RowMenuItem(RowAction.Flag, true, Newest, false), flag);
        var purge = CommandBarPresentation.MenuItem(Act(RowAction.PermanentlyDelete, confirm: true));
        Assert.True(purge?.Confirm, "a delete for good asks first, from the bar as from the menu");
        Assert.Null(CommandBarPresentation.MenuItem(new CommandItem(new Command.Sync(), true, null, false)));
    }

    /// <summary>Rules 6 and 8: the Delete key and the reading pane's Delete run whatever stands in
    /// Delete's place, which in Trash is Delete permanently.</summary>
    [Fact]
    public void DeleteIsWhateverStandsInItsPlace()
    {
        Assert.Equal(new Command.Act(RowAction.Delete), CommandBarPresentation.Delete(Bar(RowAction.Delete))?.Command);
        Assert.Equal(new Command.Act(RowAction.PermanentlyDelete), CommandBarPresentation.Delete(Bar(RowAction.PermanentlyDelete))?.Command);
        Assert.Null(CommandBarPresentation.Delete(new CommandBar([], [])));
    }

    /// <summary>The generated records hold arrays, which a record compares by reference: two equal
    /// answers must read as one, or every snapshot redraws the bar.</summary>
    [Fact]
    public void TwoAnswersDrawingTheSameBarAreTheSame()
    {
        Assert.True(CommandBarPresentation.Same(Bar(RowAction.Delete), Bar(RowAction.Delete)));
        Assert.False(CommandBarPresentation.Same(Bar(RowAction.Delete), Bar(RowAction.PermanentlyDelete)));
    }
}
