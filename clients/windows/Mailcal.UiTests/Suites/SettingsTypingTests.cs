// An account's page due to be drawn again while a name is still being typed on it
// (docs/waiting.md): the redraw waits until the field is left, so a change the core signals
// meanwhile neither sends the half-typed name nor takes the field from under the cursor.
//
// WHY THE IMAP HARNESS. The signal has to come from somewhere other than the keyboard, which is
// typing. A watched folder's switch, pressed through its peer, starts a change on the same page
// without moving the keyboard, and only an IMAP account watches folders.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ImapHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsTypingTests(ImapHarnessApp harness)
{
    private const string Name = "AccountSenderName";
    private const string Folder = "AccountWatchedFolder_Archive";

    private static Element Find(App app, Element dialog, string id) =>
        app.Agent.Find(new Filter { Id = id, Under = dialog }) ?? throw new InvalidOperationException($"the account page has no #{id}");

    [HarnessFact]
    public void AChangeSignalledWhileANameIsHalfTypedLeavesTheFieldAndSendsNothing()
    {
        string? was = null;
        string? folderWas = null;
        harness.Case(nameof(AChangeSignalledWhileANameIsHalfTypedLeavesTheFieldAndSendsNothing), app =>
        {
            var dialog = new SetupFlow(app).OpenAccount(Harness.Alice);
            // Opening the page asks the server which mailboxes it shares, and its answer draws the
            // page again: a field clicked before that is replaced under the cursor, with nothing in
            // it yet to hold the redraw. Typed into once the page has its answer.
            Assert.True(App.Eventually(() => app.Agent.FindAll(new Filter { Under = dialog })
                .Any(e => e.Id.StartsWith("SharedMailbox_", StringComparison.Ordinal)), TimeSpan.FromSeconds(30)),
                "the server's shared mailboxes reach alice's page");
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(500));
            was = app.Agent.Read(Find(app, dialog, Name)).Value ?? "";
            folderWas = app.Agent.Read(Find(app, dialog, Folder)).Toggle;
            // Never what is stored already: typing that leaves nothing unsent, and nothing holds the
            // redraw.
            var typed = was == "Half typ" ? "Half typing" : "Half typ";
            app.TypeInto(Find(app, dialog, Name), typed);
            Assert.True(app.Agent.Focused()?.Id == Name, $"the keyboard is in the name field, not #{app.Agent.Focused()?.Id}");

            app.Agent.Toggle(Find(app, dialog, Folder));
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(1500));
            // Drawn again, the field would have gone, sending what it held, and the keyboard with it.
            Assert.True(app.Agent.Focused()?.Id == Name,
                $"a change signalled while a name is being typed leaves the field where it is; the keyboard is on #{app.Agent.Focused()?.Id}");
            Assert.Equal(typed, app.Agent.Read(Find(app, dialog, Name)).Value);

            // Leaving the field sends it, and the page is drawn with both changes.
            Input.Press(Input.Tab);
            Assert.True(App.Eventually(() => app.Agent.Read(Find(app, dialog, Folder)).Toggle != folderWas, TimeSpan.FromSeconds(10)),
                "the folder's change lands once the page is drawn");
            Assert.True(App.Eventually(() => app.Agent.Read(Find(app, dialog, Name)).Value == typed, TimeSpan.FromSeconds(10)),
                "and so does the name, sent as the field was left");

        }, app =>
        {
            // Put back, however the case ended: the IMAP store is shared by every suite in its
            // collection, and a name left behind would be the next run's starting point.
            app.CloseDialogs();
            if (was is null)
            {
                return;
            }
            var dialog = new SetupFlow(app).OpenAccount(Harness.Alice);
            if (folderWas is not null && app.Agent.Read(Find(app, dialog, Folder)).Toggle != folderWas)
            {
                app.Agent.Toggle(Find(app, dialog, Folder));
                app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
            }
            var box = Find(app, dialog, Name);
            app.Agent.Focus(box);
            app.Agent.SetValue(box, was);
            app.Agent.Focus(Find(app, dialog, Folder));
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
            app.CloseDialogs();
        });
    }
}
