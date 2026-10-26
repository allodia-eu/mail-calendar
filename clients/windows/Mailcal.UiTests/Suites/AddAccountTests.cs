// Adding a second account: the setup form in a dialog over the running app, and its way back
// (docs/account-autodetect.md, rule 12). The form moves between two hosts at run time, into a
// dialog and back, and a form left in the wrong host, or a shell hidden behind a dialog that no
// longer needs it, draws without an error anywhere.
//
// The showcase has two accounts, so the add is a later one, and scripts its detection
// (autodetect_showcase.rs). Nothing connects: the form is opened, stepped through and cancelled.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class AddAccountTests(ShowcaseApp showcase)
{
    private void Case(string name, Action<App> body) => showcase.Case(name, body, Close);

    private static Element OpenForm(App app)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavAddAccount", "ListItem")) ?? throw new InvalidOperationException("the sidebar has no Add account row"));
        return app.Agent.Wait(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Add account opened no setup form (#DetectEmail)");
    }

    private static void Close(App app)
    {
        if ((app.Agent.Find(Filter.ById("DetectCancelButton", "Button")) ?? app.Agent.Find(Filter.ById("CancelButton", "Button"))) is { } cancel)
        {
            app.Agent.Invoke(cancel);
        }
        app.CloseDialogs();
    }

    /// <summary>Opens the form, continues with <paramref name="address"/>, and waits for the card
    /// detection found.</summary>
    private static void OpenCard(App app, string address)
    {
        app.Agent.SetValue(OpenForm(app), address);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ContinueButton", "Button"))!);
        if (app.Agent.Wait(Filter.ById("ConnectButton", "Button"), TimeSpan.FromSeconds(15)) is null)
        {
            throw new InvalidOperationException($"continuing with {address} reached no card");
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
    }

    private static Element? Shown(App app, string id) => app.Agent.Find(Filter.ById(id));

    private static bool On(App app, string id) => Shown(app, id) is { } box && app.Agent.Read(box).Toggle == "On";

    [OfflineFact]
    public void TheFormOpensOverTheShellWhichStaysWhereItWas() => Case(nameof(TheFormOpensOverTheShellWhichStaysWhereItWas), app =>
    {
        OpenForm(app);
        // Behind a dialog the shell is still drawn; collapsed for an add, it once took the whole
        // window for a form a few hundred pixels wide.
        Assert.True(Shown(app, "SettingsItem") is not null, "the sidebar stays on screen behind the add-account dialog");
        Assert.True(Shown(app, "AddAccountDialog") is not null, "the form is hosted in the add-account dialog");
    });

    [OfflineFact]
    public void TheAddressStepCanBeCancelledAndCancellingClosesTheDialog() => Case(nameof(TheAddressStepCanBeCancelledAndCancellingClosesTheDialog), app =>
    {
        OpenForm(app);
        app.Agent.Invoke(Shown(app, "DetectCancelButton") ?? throw new InvalidOperationException("a later add opens on the address step, which needs its own way out"));
        Assert.True(app.Agent.WaitGone(Filter.ById("DetectEmail"), TimeSpan.FromSeconds(5)), "Cancel on the address step closes the dialog");
    });

    [OfflineFact]
    public void TheSecondStepGoesBackToTheAddressInsideTheDialog() => Case(nameof(TheSecondStepGoesBackToTheAddressInsideTheDialog), app =>
    {
        const string address = "someone@example.com";
        app.Agent.SetValue(OpenForm(app), address);
        app.Agent.Invoke(Shown(app, "ManualButton")!);
        Assert.True(app.Agent.Wait(Filter.ById("ImapHost", "Edit"), TimeSpan.FromSeconds(5)) is not null, "Set up manually opens the manual form");
        var back = Shown(app, "BackButton");
        Assert.True(back is { Enabled: true }, "the manual form has a way back to the address step");
        app.Agent.Invoke(back!);
        var box = app.Agent.Wait(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("Back closed the dialog instead of returning to the address step");
        Assert.Equal(address, app.Agent.Read(box).Value);
    });

    [OfflineFact]
    public void TheFoundCardOffersMailTheCalendarAndContactsAsDetectionFoundThem() => Case(nameof(TheFoundCardOffersMailTheCalendarAndContactsAsDetectionFoundThem), app =>
    {
        // northwind.example publishes its mail and a calendar, and contacts are looked for at the
        // calendar's server.
        OpenCard(app, "eva@northwind.example");
        foreach (var use in new[] { "SetupUseMail", "SetupUseCalendar", "SetupUseContacts" })
        {
            Assert.True(On(app, use), $"a use whose server was found starts on (rule 8): #{use}");
        }
        Assert.True(Shown(app, "SetupUseCalendarServer") is null, "a calendar that was found is named by its host, not asked for");
        Assert.True(Shown(app, "SetupMailServerIMAP") is not null, "the mail server that was found is named too");
        foreach (var field in new[] { "ImapHost", "ImapPort", "SmtpHost", "Username" })
        {
            Assert.True(Shown(app, field) is null, $"detected servers are shown to confirm, not to retype (#{field}): the manual form edits them");
        }
        app.Agent.SetToggle(Shown(app, "SetupUseMail")!, on: false);
        Assert.True(app.Agent.WaitGone(Filter.ById("SetupMailServerIMAP"), TimeSpan.FromSeconds(3)), "the mail servers follow the mail toggle");
        app.Agent.SetToggle(Shown(app, "SetupUseCalendar")!, on: false);
        app.Agent.Settle();
        var contacts = Shown(app, "SetupUseContacts")!;
        Assert.True(On(app, "SetupUseContacts") && !contacts.Enabled, "the last use on is held on: an account used for nothing is not one");
        app.Agent.SetToggle(Shown(app, "SetupUseMail")!, on: true);
        Assert.True(app.Agent.Wait(Filter.ById("SetupMailServerIMAP"), TimeSpan.FromSeconds(3)) is not null, "switching mail back on brings its servers back");
        app.Agent.Invoke(Shown(app, "SetupManualButton") ?? throw new InvalidOperationException("the found card offers no way to set up manually"));
        Assert.True(app.Agent.Wait(Filter.ById("ImapHost", "Edit"), TimeSpan.FromSeconds(5)) is not null, "\"Set up manually\" from the found card offers its servers to edit");
    });

    [OfflineFact]
    public void ACalendarTheDomainDoesNotPublishStartsOffAndSwitchedOnAsksForItsServer() => Case(nameof(ACalendarTheDomainDoesNotPublishStartsOffAndSwitchedOnAsksForItsServer), app =>
    {
        // oldschool.example publishes its mail and no calendar.
        OpenCard(app, "eva@oldschool.example");
        Assert.True(Shown(app, "SetupUseCalendar") is not null && !On(app, "SetupUseCalendar"), "a use whose server was not found starts off (rule 8)");
        Assert.True(Shown(app, "SetupUseCalendarServer") is null, "and asks for nothing while off");
        app.Agent.SetToggle(Shown(app, "SetupUseCalendar")!, on: true);
        Assert.True(app.Agent.Wait(Filter.ById("SetupUseCalendarServer"), TimeSpan.FromSeconds(3)) is not null, "switching it on asks for the server");
    });

    [OfflineFact]
    public void ADomainWithACalendarAndAddressBookAndNoMailGetsACardOfItsOwn() => Case(nameof(ADomainWithACalendarAndAddressBookAndNoMailGetsACardOfItsOwn), app =>
    {
        // cloud.example publishes CalDAV and CardDAV and no mail server.
        OpenCard(app, "eva@cloud.example");
        Assert.True(Shown(app, "SetupUseMail") is null, "the calendar-and-contacts route offers no mail (rule 8)");
        Assert.True(On(app, "SetupUseCalendar") && On(app, "SetupUseContacts"), "both were found, so both start on");
        Assert.True(Shown(app, "ImapHost") is null, "the card draws no mail server");
        Assert.True(Shown(app, "DavLogin") is null, "the typed address is the login, so the card does not ask for one");
        Assert.True(Shown(app, "DavPassword") is not null, "it asks for the password alone");
    });
}
