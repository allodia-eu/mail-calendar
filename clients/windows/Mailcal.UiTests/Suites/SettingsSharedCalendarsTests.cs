// The calendars in the Shared mailboxes and calendars section of an account's page in Settings →
// Accounts (docs/shared-mailboxes.md rules 3, 7, 10 and 11): what the server lists is a switch per
// store, a store holding a mailbox and a calendar is one switch, one switched on is an account of
// its own with its server's rights, and switching it off is asked first and takes it away again.
//
// WHY HERE. The core decides the section and its tests pin it, but whether the page draws the
// server's answer as one switch per store, sends the store's intent and shows what the next
// snapshot brings is WinUI and the core together.
//
// The launch is stalwart: alice over JMAP, a member of the `support` group, whose calendar she may
// write, and dave shares a calendar of his with her read-only (docker/stalwart/README.md). Each case
// starts from both switched off and its cleanup leaves them so, since the store outlives the launch.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsSharedCalendarsTests(HarnessApp harness)
{
    private const string Dave = "dave@test.local";
    private const string Support = "support@test.local";

    private static string SwitchId(string store) => $"SharedMailbox_{store}";

    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app =>
        {
            app.CloseDialogs();
            SwitchOff(app, new SetupFlow(app), Dave);
            SwitchOff(app, new SetupFlow(app), Support);
            app.CloseDialogs();
        });

    /// <summary>The store's switch on alice's page, once her server has answered.</summary>
    private static Element Switch(App app, Element dialog, string store) =>
        app.Agent.Wait(new Filter { Id = SwitchId(store), Under = dialog }, TimeSpan.FromSeconds(30))
            ?? throw new InvalidOperationException(
                $"alice's page drew no calendar switch for {store}, which her server lists. Note: {Note(app, dialog) ?? "(none)"}");

    private static string? Note(App app, Element dialog) =>
        app.Agent.Find(new Filter { Id = "SharedMailboxesNote", Under = dialog })?.Name;

    private static bool IsOn(App app, Element dialog, string store) => app.Agent.Read(Switch(app, dialog, store)).Active == true;

    private static Element Button(App app, Element dialog, string id) =>
        app.Agent.Wait(new Filter { Id = id, Type = "Button", Under = dialog }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException($"the page offers no #{id}");

    /// <summary>The calendars of a shared store Settings → Calendar offers a new event to: a
    /// shared calendar is an account of its own, under an id derived from alice's.</summary>
    private static List<Element> SharedDefaults(App app, SetupFlow setup) =>
        app.Agent.FindAll(new Filter { Type = "RadioButton", Under = setup.OpenCategory("calendar") })
            .Where(radio => radio.Id.StartsWith("DefaultCalendar_", StringComparison.Ordinal)
                && radio.Id.Contains("#shared:", StringComparison.Ordinal))
            .ToList();

    /// <summary>Leaves <paramref name="store"/> switched off, through the page, if it is on.</summary>
    private static void SwitchOff(App app, SetupFlow setup, string store)
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        if (app.Agent.Wait(new Filter { Id = SwitchId(store), Under = dialog }, TimeSpan.FromSeconds(30)) is not { } toggle
            || app.Agent.Read(toggle).Active != true)
        {
            return;
        }
        app.Agent.SetToggle(toggle, on: false);
        app.Agent.Invoke(Button(app, dialog, "SharedMailboxOffConfirm"));
        App.Eventually(() => !IsOn(app, dialog, store), TimeSpan.FromSeconds(20));
    }

    [HarnessFact]
    public void TheServersCalendarsAreListedAndNothingIsChosenForThePerson() => Case(nameof(TheServersCalendarsAreListedAndNothingIsChosenForThePerson), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        Assert.False(IsOn(app, dialog, Dave), "a listed calendar starts off: nothing is chosen for the person (rule 3)");
        Assert.False(IsOn(app, dialog, Support), "a listed calendar starts off: nothing is chosen for the person (rule 3)");
        Assert.True(Note(app, dialog) is null, $"every calendar the server listed is a switch, so no line explains an absence; it reads: {Note(app, dialog)}");
        // Rule 10: the group holds a mailbox and a calendar, and they are one store with one switch.
        Assert.True(app.Agent.FindAll(new Filter { Id = SwitchId(Support), Under = dialog }).Count == 1,
            "the support group's mailbox and calendar are one switch");
    });

    [HarnessFact]
    public void SwitchedOnEachIsACalendarOfItsOwnWithItsServersRights() => Case(nameof(SwitchedOnEachIsACalendarOfItsOwnWithItsServersRights), (app, setup) =>
    {
        Assert.Empty(SharedDefaults(app, setup));
        var dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog, Support), on: true);
        Assert.True(App.Eventually(() => IsOn(app, dialog, Support), TimeSpan.FromSeconds(30)),
            "the page is drawn again from what the server answers after the switch, and it is on");
        app.Agent.SetToggle(Switch(app, dialog, Dave), on: true);
        Assert.True(App.Eventually(() => IsOn(app, dialog, Dave), TimeSpan.FromSeconds(30)),
            "the page is drawn again from what the server answers after the switch, and it is on");
        // Rule 11: each is a calendar account of its own, with the rights its server gives. Alice
        // may write the group's calendar, so a new event can be filed there; dave shares his
        // read-only, so it takes none and is not offered.
        var offered = new List<Element>();
        Assert.True(App.Eventually(() => (offered = SharedDefaults(app, setup)).Count > 0, TimeSpan.FromSeconds(60)),
            "the support calendar is switched on and Settings → Calendar offers no new event to it (rule 11)");
        Assert.True(offered.Count == 1,
            $"only the calendar alice may write takes a new event; offered: {string.Join(" | ", offered.Select(radio => radio.Name))}");
    });

    [HarnessFact]
    public void SwitchingItOffIsAskedFirstAndTakesItAway() => Case(nameof(SwitchingItOffIsAskedFirstAndTakesItAway), (app, setup) =>
    {
        var dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog, Support), on: true);
        Assert.True(App.Eventually(() => IsOn(app, dialog, Support), TimeSpan.FromSeconds(30)), "the calendar could not be switched on");
        Assert.True(App.Eventually(() => SharedDefaults(app, setup).Count == 1, TimeSpan.FromSeconds(60)),
            "the support calendar is switched on and Settings → Calendar does not offer it");

        dialog = setup.OpenAccount(Harness.Alice);
        app.Agent.SetToggle(Switch(app, dialog, Support), on: false);
        // Rule 7: switching it off deletes what the device holds of it, so it is confirmed first.
        Button(app, dialog, "SharedMailboxOffConfirm");
        app.Agent.Invoke(Button(app, dialog, "SharedMailboxOffCancel"));
        Assert.True(App.Eventually(() => IsOn(app, dialog, Support), TimeSpan.FromSeconds(5)), "cancelling leaves the calendar on");

        app.Agent.SetToggle(Switch(app, dialog, Support), on: false);
        app.Agent.Invoke(Button(app, dialog, "SharedMailboxOffConfirm"));
        Assert.True(App.Eventually(() => !IsOn(app, dialog, Support), TimeSpan.FromSeconds(20)), "confirmed, the calendar is switched off");
        Assert.True(App.Eventually(() => SharedDefaults(app, setup).Count == 0, TimeSpan.FromSeconds(20)),
            "a calendar switched off is no longer offered a new event");
    });
}
