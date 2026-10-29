// The Shared mailboxes section of an account's page in Settings → Accounts
// (docs/shared-mailboxes.md rules 3, 5 and 7): what the server lists is a switch per mailbox, one
// switched on is a mail account of its own in the folder pane with its own mail, a new message from
// it is from its address, and switching it off is asked first and takes it away again.
//
// WHY HERE. The rules are pinned in Mailcal.Tests (SharedMailboxRulesTests), but whether the page
// asks the server off the UI thread, draws the answer into the page it opened and hands the press
// to the core, which only the next snapshot shows, is WinUI and the core together.
//
// The launch is stalwart: alice over JMAP, a member of the harness's `support` group, so her
// server lists support@test.local. Each case starts from the mailbox switched off and its cleanup
// leaves it so, whatever happens, since the store outlives the launch and every other harness suite
// counts alice's mailboxes.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsSharedMailboxesTests(HarnessApp harness)
{
    private const string Support = "support@test.local";
    private const string Seeded = "Order 4711 arrived damaged";
    private const string SwitchId = "SharedMailbox_support@test.local";

    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app =>
        {
            app.CloseComposer();
            app.CloseDialogs();
            SwitchOff(app, new SetupFlow(app));
            app.CloseDialogs();
        });

    /// <summary>The support mailbox's switch on alice's page, once her server has answered.</summary>
    private static Element Switch(App app, Element dialog) =>
        app.Agent.Wait(new Filter { Id = SwitchId, Under = dialog }, TimeSpan.FromSeconds(30))
            ?? throw new InvalidOperationException(
                $"alice's page drew no switch for {Support}, which her server lists. Note: {Note(app, dialog) ?? "(none)"}");

    private static string? Note(App app, Element dialog) =>
        app.Agent.Find(new Filter { Id = "SharedMailboxesNote", Under = dialog })?.Name;

    private static bool IsOn(App app, Element dialog) => app.Agent.Read(Switch(app, dialog)).Toggle == "On";

    /// <summary>The pane's row for the support mailbox, which is named by its address or by the
    /// name its server holds for it.</summary>
    private static Element? PaneRow(App app) =>
        app.Agent.FindAll(new Filter { Type = "ListItem", Window = app.Mailbox.Hwnd })
            .FirstOrDefault(row => row.Name.Contains("support", StringComparison.OrdinalIgnoreCase));

    private static Element Button(App app, Element dialog, string id) =>
        app.Agent.Wait(new Filter { Id = id, Type = "Button", Under = dialog }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException($"the page offers no #{id}");

    /// <summary>Leaves the support mailbox switched off, through the page, if it is on.</summary>
    private static void SwitchOff(App app, SetupFlow setup)
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        if (app.Agent.Wait(new Filter { Id = SwitchId, Under = dialog }, TimeSpan.FromSeconds(30)) is not { } toggle
            || app.Agent.Read(toggle).Toggle != "On")
        {
            return;
        }
        app.Agent.SetToggle(toggle, on: false);
        app.Agent.Invoke(Button(app, dialog, "SharedMailboxOff"));
        App.Eventually(() => !IsOn(app, dialog), TimeSpan.FromSeconds(20));
    }

    [HarnessFact]
    public void TheServersMailboxIsListedAndNothingIsChosenForThePerson() => Case(nameof(TheServersMailboxIsListedAndNothingIsChosenForThePerson), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        Assert.False(IsOn(app, dialog), "a listed mailbox starts off: nothing is chosen for the person (rule 3)");
        Assert.True(Note(app, dialog) is null, $"every mailbox the server listed is a switch, so no line explains an absence; it reads: {Note(app, dialog)}");
        app.CloseDialogs();
        Assert.True(PaneRow(app) is null, "a mailbox not switched on is in no mail surface");
    });

    [HarnessFact]
    public void SwitchedOnItIsAMailboxOfItsOwnSendingFromItsAddress() => Case(nameof(SwitchedOnItIsAMailboxOfItsOwnSendingFromItsAddress), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog), on: true);
        Assert.True(App.Eventually(() => IsOn(app, dialog), TimeSpan.FromSeconds(20)),
            "the page is drawn again from what the server answers after the switch, and it is on");
        app.CloseDialogs();

        var account = app.Agent.Wait(new Filter { Type = "ListItem", Window = app.Mailbox.Hwnd, NameLike = "*upport*" }, App.Patience)
            ?? throw new InvalidOperationException("the support mailbox is switched on and the folder pane has no row for it (rule 5)");
        app.Agent.Expand(account);
        var inbox = app.Agent.Wait(Filter.Named(Catalog.English("folder_inbox"), "ListItem") with { Under = account }, TimeSpan.FromSeconds(60))
            ?? throw new InvalidOperationException("the support mailbox lists no Inbox of its own");
        app.Agent.Invoke(inbox);
        app.RowTitled(Seeded);

        new DraftsFlow(app).OpenNewComposer();
        var from = app.Agent.Find(Filter.ById("FromBox", "ComboBox")) ?? throw new InvalidOperationException("the composer has no From picker (#FromBox)");
        app.Agent.Expand(from);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var selected = app.Agent.Read(from).Selection ?? "";
        app.Agent.Collapse(from);
        Assert.True(selected.Contains(Support, StringComparison.Ordinal),
            $"a new message from the support mailbox is from its address, not alice's (rule 5); From reads: {selected}");
    });

    [HarnessFact]
    public void AChosenMailboxIsAMailboxOfItsOwnAgainAfterARelaunch() => Case(nameof(AChosenMailboxIsAMailboxOfItsOwnAgainAfterARelaunch), (app, setup) =>
    {
        // Rule 4: chosen, it is stored with the account that opens it, and rule 6: a launch dials it
        // again with that account's sign-in.
        var dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog), on: true);
        Assert.True(App.Eventually(() => IsOn(app, dialog), TimeSpan.FromSeconds(20)), "the mailbox could not be switched on");
        app.CloseDialogs();

        harness.RelaunchOnHarness();
        app = harness.App;
        Assert.True(app.Agent.Wait(new Filter { Type = "ListItem", Window = app.Mailbox.Hwnd, NameLike = "*upport*" }, App.Patience) is not null,
            "the support mailbox was switched on before the relaunch, and the folder pane after it has no row for it");
        Assert.True(IsOn(app, new SetupFlow(app).OpenAccount(Harness.Alice)), "alice's page still has the mailbox switched on");
    });

    [HarnessFact]
    public void SwitchingItOffIsAskedFirstAndTakesItAway() => Case(nameof(SwitchingItOffIsAskedFirstAndTakesItAway), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog), on: true);
        Assert.True(App.Eventually(() => IsOn(app, dialog), TimeSpan.FromSeconds(20)), "the mailbox could not be switched on");
        app.CloseDialogs();
        Assert.True(app.Agent.Wait(new Filter { Type = "ListItem", Window = app.Mailbox.Hwnd, NameLike = "*upport*" }, App.Patience) is not null,
            "the support mailbox is switched on and the folder pane has no row for it");

        dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog), on: false);
        // Rule 7: switching it off deletes what the device holds of it, so it is confirmed first.
        Button(app, dialog, "SharedMailboxOff");
        app.Agent.Invoke(Button(app, dialog, "SharedMailboxOffCancel"));
        Assert.True(App.Eventually(() => IsOn(app, dialog), TimeSpan.FromSeconds(5)), "cancelling leaves the mailbox on");

        app.Agent.SetToggle(Switch(app, dialog), on: false);
        app.Agent.Invoke(Button(app, dialog, "SharedMailboxOff"));
        Assert.True(App.Eventually(() => !IsOn(app, dialog), TimeSpan.FromSeconds(20)), "confirmed, the mailbox is switched off");
        app.CloseDialogs();
        Assert.True(App.Eventually(() => PaneRow(app) is null, TimeSpan.FromSeconds(20)),
            "a mailbox switched off leaves the folder pane");
    });
}
