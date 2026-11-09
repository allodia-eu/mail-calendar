// Delete in Trash (docs/command-bar.md, rules 6 and 8): the bar's Delete is Delete permanently there,
// and it is asked first, from the bar and from the reading pane, whose Delete is the bar's over the
// open message. Outside Trash it is Delete, which CommandBarTests sees on the showcase.
//
// WHY THE HARNESS. Only a real Trash holds a message the bar can be asked about; the showcase
// performs no mail action, so nothing it deletes arrives in its Trash. The message is filed in
// alice's Trash on the server, as one thrown away on another device would be, and everything from
// there on is the app's own path. The case confirms the second question, so it ends with the
// message gone, and its cleanup removes it on the server whatever happened.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class CommandBarTrashTests(HarnessApp harness)
{
    private const string Subject = "Thrown away on another device";

    private static Element Delete(App app) =>
        app.Agent.Wait(new Filter { Id = "CommandDelete", Type = "Button", Window = app.Mailbox.Hwnd, OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the command bar draws no #CommandDelete");

    private static bool Listed(App app) =>
        app.Rows().Any(row => row.Name.Contains(Subject, StringComparison.Ordinal));

    /// <summary>The question before a delete for good, checked to be that question; answered with its
    /// confirm button when <paramref name="confirm"/>, else with Cancel.</summary>
    private static void Answer(App app, string from, bool confirm)
    {
        var dialog = app.Dialog(Filter.ById("PrimaryButton", "Button"));
        Assert.True(app.Agent.FindAll(new Filter { Under = dialog }).Any(e => e.Name == Catalog.English("delete_permanently_title")),
            $"{from}'s Delete permanently opened something other than the question before a delete for good");
        var primary = app.Agent.Find(new Filter { Id = "PrimaryButton", Type = "Button", Under = dialog })!;
        Assert.Equal(Catalog.English("action_delete_permanently"), primary.Name);
        app.Agent.Invoke(confirm ? primary : app.Agent.Find(new Filter { Id = "CloseButton", Type = "Button", Under = dialog })!);
        app.CloseDialogs();
    }

    [HarnessFact]
    public void InTrashDeleteIsDeletePermanentlyAndIsAskedFirstFromTheBarAndTheReadingPane() =>
        harness.Case(nameof(InTrashDeleteIsDeletePermanentlyAndIsAskedFirstFromTheBarAndTheReadingPane), app =>
        {
            Harness.PutInTrash(Harness.Alice, Subject);
            app.OpenFolder("Trash");
            if (!App.Eventually(() =>
                {
                    app.Agent.Invoke(app.Agent.Find(Filter.ById("CommandSync", "Button"))!);
                    app.Agent.Settle(floor: TimeSpan.FromSeconds(1));
                    return Listed(app);
                }, TimeSpan.FromSeconds(40)))
            {
                throw new InvalidOperationException($"'{Subject}' was filed in Trash on the server and never listed");
            }
            app.OpenMessage(Subject);
            var delete = Delete(app);
            App.Eventually(() => (delete = Delete(app)).Name == Catalog.English("action_delete_permanently"), TimeSpan.FromSeconds(5));
            Assert.Equal(Catalog.English("action_delete_permanently"), delete.Name);
            Assert.True(delete.Enabled, "Delete permanently over a message in Trash responds");

            app.Agent.Invoke(delete);
            Answer(app, "the bar", confirm: false);
            Assert.True(Listed(app), "Cancel deleted the message anyway");

            app.Agent.Invoke(app.Agent.Find(Filter.ById("ReadingDelete", "Button") with { OnScreen = true })
                ?? throw new InvalidOperationException("the reading pane draws no #ReadingDelete"));
            Answer(app, "the reading pane", confirm: true);
            Assert.True(App.Eventually(() => !Listed(app), TimeSpan.FromSeconds(15)), "the confirmed delete left the message in the list");
            Assert.True(App.Eventually(() => Harness.CountMail(Harness.Alice, Subject) == 0, TimeSpan.FromSeconds(30)),
                "and the server still holds it: the delete for good never reached it");
        }, app =>
        {
            app.CloseDialogs();
            Harness.RemoveMail(Harness.Alice, Subject);
            // The launch is shared, and the suites after this one read the Inbox.
            app.OpenFolder("Inbox");
        });
}
