// A mailbox whose calendar and contacts are on a Nextcloud server its own password does not open
// (docs/nextcloud-sign-in.md rule 2, docs/accounts.md rule 18): the separate-server step after the
// connect offers Nextcloud's own sign-in above its password field, and the sign-in moves them to an
// account of their own, linked to the mailbox. A use skipped there and switched back on later is
// offered the same form from its row on the account's page.
//
// WHY HERE. Which sign-in the form offers is the core's, unit-tested in mailcal-bindings.
// What it cannot see is the step reached at all from a refused calendar, the answer drawn above the
// login, the grant completed into a linked account, the page's action giving way to its form, and a
// sign-in the person cancelled, replaced or walked away from staying over when its grant arrives.
//
// The mailbox is alice's on the Stalwart harness over IMAP; its calendar server is the engine's
// Nextcloud fixture (docker/nextcloud in email-calendar-sync-engine, 127.0.0.1:18082), which refuses
// the harness password, as any separate server would. The sign-in opens the default browser on the
// fixture's login page, and Driver/NextcloudBrowser.cs does in its place what the person does there.
// The cases run in the order written; the last removes both accounts. The fixture slows and then
// refuses every sign-in from this host as refused passwords mount up, and each connect here sends it
// one, so each case starts with them forgotten (NextcloudBrowser.ForgetRefusals).

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "harness")]
public sealed class NextcloudSeparateServerTests(SetupHarnessApp harness) : IClassFixture<SetupHarnessApp>
{
    private const string Address = "alice@test.local";

    // A second harness mailbox, for the add-account dialog, which only opens over a mailbox.
    private const string Bob = "bob@test.local";

    // The fixture's own login for alice, which the form is given.
    private const string SeparateLogin = "alice";

    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => Fresh(app, body), app => app.CloseDialogs(), reset: false);

    /// <summary>A case that sets the mailbox up and removes it again whatever happened, so the
    /// cases after it start from the first-run screen.</summary>
    private void Removing(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => Fresh(app, body), app => RemoveAll(app, new SetupFlow(app)), reset: false);

    private static void Fresh(App app, Action<App, SetupFlow> body)
    {
        NextcloudBrowser.ForgetRefusals();
        body(app, new SetupFlow(app));
    }

    private static Element Field(App app, string id) =>
        app.Agent.Find(Filter.ById(id, "Edit")) ?? throw new InvalidOperationException($"the form has no #{id}");

    private static Element? InPage(App app, Element dialog, string id) =>
        app.Agent.Find(new Filter { Id = id, Under = dialog });

    /// <summary>Sets a harness mailbox up over IMAP, alice's unless told otherwise, with its calendar
    /// at the fixture, as far as the separate-server step the fixture's refusal leads to.</summary>
    private static void ConnectToTheSeparateServerStep(
        App app, SetupFlow flow, string address = Address, string mailPassword = "harness-alice-pw")
    {
        flow.OpenManualSetup(address, Filter.ById("ImapChoice"));
        app.Agent.SetValue(Field(app, "ImapPort"), "12993");
        app.Agent.SetValue(Field(app, "ImapHost"), "localhost");
        app.Agent.SetValue(Field(app, "Username"), address);
        app.Agent.SetValue(Field(app, "SmtpPort"), "12465");
        app.Agent.SetValue(Field(app, "SmtpHost"), "localhost");
        app.Agent.SetValue(Field(app, "CaldavUrl"), $"{NextcloudAccountPageTests.Fixture}/remote.php/dav/");
        Element? password = null;
        Assert.True(App.Eventually(() => (password = app.Agent.Find(Filter.ById("Password", "Edit"))) is { Width: > 0 }, TimeSpan.FromSeconds(30)),
            "the harness mailbox takes a password");
        app.Agent.SetValue(password!, mailPassword);
        Element? connect = null;
        App.Eventually(() => (connect = app.Agent.Find(Filter.ById("ConnectButton", "Button"))) is { Enabled: true }, TimeSpan.FromSeconds(10));
        app.Agent.Invoke(connect ?? throw new InvalidOperationException("the form never let Connect go"));

        Assert.True(app.Agent.Wait(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(60)) is not null,
            "accounts.md rule 18: a calendar whose server refused the mailbox's password leads to the separate-server step");
    }

    /// <summary>Skips the separate-server step, then switches the calendar back on from the account's
    /// page and waits for the sign-in action its refusal offers. Returns the page's dialog.</summary>
    private static Element SkipThenSwitchTheCalendarBackOn(App app, SetupFlow flow)
    {
        var skip = app.Agent.Find(Filter.ById("SeparateServerSecondary", "Button"))
            ?? throw new InvalidOperationException("the separate-server step offers no Skip");
        app.Agent.Invoke(skip);
        Assert.True(app.Agent.WaitGone(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(30)),
            "Skip leaves the separate-server step");
        flow.CompleteSetupPrompts();

        var dialog = flow.OpenAccount(Address);
        app.Agent.SetToggle(InPage(app, dialog, "AccountUse_Calendar")
            ?? throw new InvalidOperationException("the account page has no calendar switch"), true);
        Assert.True(App.Eventually(() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is { Enabled: true }, TimeSpan.FromSeconds(60)),
            "rule 18: a calendar whose server refused the mailbox's password offers a sign-in of its own on its row");
        return dialog;
    }

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void AWrongPasswordIsHeldThenRefusedOnTheStepAndThePageAndARightOneClosesThePagesForm() =>
        Removing(nameof(AWrongPasswordIsHeldThenRefusedOnTheStepAndThePageAndARightOneClosesThePagesForm), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            AssertAWrongPasswordIsHeldThenRefused(app, null, "the setup step");

            // Skip stops using the calendar, as a change on the page, and the step gives way to the
            // link step. It touches no server, so it ends before a ring could be sampled.
            var dialog = SkipThenSwitchTheCalendarBackOn(app, flow);
            // The calendar is the one use the fixture's server holds, so the action is under its row
            // and under no other. Read off the rows rather than the screen: the page lays its rows
            // out below the fold, where a rectangle reads empty.
            Assert.Null(InPage(app, dialog, "AccountUseSeparateSignIn_Contacts"));
            Assert.Null(InPage(app, dialog, "AccountUseSeparateSignIn_Mail"));
            // Looked up again: the page is drawn afresh while the switch lands.
            app.Agent.Invoke(InPage(app, dialog, "AccountUseSeparateSignIn_Calendar")
                ?? throw new InvalidOperationException("the calendar's sign-in action went away before it was pressed"));
            if (app.Agent.Wait(new Filter { Id = "SeparateServerConnect", Under = dialog }, TimeSpan.FromSeconds(10)) is null)
            {
                throw new InvalidOperationException("the action opened no form");
            }
            AssertAWrongPasswordIsHeldThenRefused(app, dialog, "the account's page");

            app.Agent.SetValue(InPage(app, dialog, "SeparateServerPassword")!, "nextcloud-alice-pw");
            app.Agent.Invoke(InPage(app, dialog, "SeparateServerConnect")!);
            Assert.True(app.Agent.WaitGone(new Filter { Id = "SeparateServerConnect", Under = dialog }, TimeSpan.FromSeconds(60)),
                "a password the server takes moves the calendar to an account of its own, and the form closes");
            // Watched for a while rather than read once: the form is gone before the page is drawn
            // again, so a single read finds no action whether or not one is coming back.
            app.Agent.Settle();
            Assert.False(App.Eventually(() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is not null, TimeSpan.FromSeconds(5)),
                "and the action does not come back: nothing waits on a sign-in any more");
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void AUseSwitchedBackOnOffersItsOwnSignInFromItsRowAndItsFormTakesTheActionsPlace() =>
        Removing(nameof(AUseSwitchedBackOnOffersItsOwnSignInFromItsRowAndItsFormTakesTheActionsPlace), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            var dialog = SkipThenSwitchTheCalendarBackOn(app, flow);
            app.Agent.Invoke(InPage(app, dialog, "AccountUseSeparateSignIn_Calendar")
                ?? throw new InvalidOperationException("the calendar's sign-in action went away before it was pressed"));
            Assert.True(app.Agent.Wait(new Filter { Id = "SeparateServerConnect", Under = dialog }, TimeSpan.FromSeconds(10)) is not null,
                "the action opens the form under its row");
            Assert.True(App.Eventually(() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is null, TimeSpan.FromSeconds(5)),
                "the action is hidden while its form is open");

            app.Agent.Invoke(InPage(app, dialog, "SeparateServerSecondary")
                ?? throw new InvalidOperationException("the page's form offers no Cancel"));
            Assert.True(App.Eventually(() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is { Enabled: true }
                && InPage(app, dialog, "SeparateServerConnect") is null, TimeSpan.FromSeconds(10)),
                "Cancel closes the form and puts the action back");
        });

    /// <summary>
    /// Connects the form under <paramref name="root"/> (the window, or the account's page) with a
    /// password the fixture refuses, and asserts what docs/waiting.md asks of it: Connect's ring and
    /// the fields held while it runs, then the refusal under the fields with the login kept.
    /// </summary>
    private static void AssertAWrongPasswordIsHeldThenRefused(App app, Element? root, string where)
    {
        Element InForm(string id) =>
            app.Agent.Find(new Filter { Id = id, Under = root }) ?? throw new InvalidOperationException($"{where}'s form has no #{id}");
        app.Agent.SetValue(InForm("SeparateServerUsername"), SeparateLogin);
        app.Agent.SetValue(InForm("SeparateServerPassword"), "not-alices-password");
        Element? connect = null;
        Assert.True(App.Eventually(() => (connect = app.Agent.Find(new Filter { Id = "SeparateServerConnect", Under = root })) is { Enabled: true }, TimeSpan.FromSeconds(5)),
            $"{where}: a login and password let Connect go");
        app.Agent.Invoke(connect!);

        // Sampled until the answer is drawn: the fixture takes a moment to refuse a password.
        var held = false;
        // Read off the tree rather than the screen: the account's page lays the form out below the
        // fold, where every rectangle reads empty.
        bool Refused() => app.Agent.Find(new Filter { Id = "SeparateServerError", Under = root }) is { } error
            && app.Agent.FindAll(new Filter { Under = error }).Any(e => e.Name.Length > 0);
        Assert.True(App.Eventually(() =>
        {
            // Connect's ring is read where the form is on screen (setup's step): below the fold, a
            // ring and a collapsed one both read empty. The fields held are read on both.
            held |= (root is not null || app.Agent.Find(new Filter { Id = "SeparateServerConnect_Running" }) is { Rendered: true })
                && app.Agent.Find(new Filter { Id = "SeparateServerPassword", Under = root }) is { Enabled: false }
                && app.Agent.Find(new Filter { Id = "SeparateServerConnect", Under = root }) is { Enabled: false };
            return Refused();
        }, TimeSpan.FromSeconds(60)), $"{where}: a password the server refuses is said under the fields; the error holds: "
            + string.Join(" | ", app.Agent.Find(new Filter { Id = "SeparateServerError", Under = root }) is { } shown
                ? app.Agent.FindAll(new Filter { Under = shown }).Select(e => $"{e.Type} '{e.Name}'")
                : ["no error element"]));
        Assert.True(held, $"{where}: while Connect ran, the fields and Connect were held{(root is null ? ", with Connect's ring showing" : "")} (docs/waiting.md)");
        Assert.Equal(SeparateLogin, app.Agent.Read(InForm("SeparateServerUsername")).Value);
        Assert.True(InForm("SeparateServerPassword").Enabled, $"{where}: the fields are handed back for another try");
    }

    /// <summary>Removes alice's mailbox and every account beside it, the linked calendar account
    /// first, and waits for the first-run screen.</summary>
    private static void RemoveAll(App app, SetupFlow flow)
    {
        app.CloseDialogs();
        if (app.Agent.Find(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }) is not null)
        {
            return;
        }
        // A page just opened is rebuilt by the snapshots that follow it, and a press on the Remove
        // it replaced lands on a button no longer on screen, so the page never asks. Press the one
        // on screen until it does.
        void Remove(Element dialog)
        {
            app.Agent.Settle();
            Element? confirm = null;
            var asked = App.Eventually(() =>
            {
                if ((confirm = app.Agent.Find(new Filter { Id = "AccountRemoveConfirm", Under = dialog })) is not null)
                {
                    return true;
                }
                app.Agent.Invoke(app.Agent.Find(new Filter { Id = "AccountRemove", Under = dialog })
                    ?? throw new InvalidOperationException("the account's page has neither Remove nor its confirmation"));
                app.Agent.Settle();
                return false;
            }, TimeSpan.FromSeconds(15));
            app.Agent.Invoke(asked && confirm is not null
                ? confirm
                : throw new InvalidOperationException("Remove asked nothing before removing the account"));
            app.Agent.Settle();
        }
        if (flow.AccountRows().Any(row => row.Name == Bob))
        {
            Remove(flow.OpenAccount(Bob));
        }
        while (flow.AccountRows().Count(row => row.Name == Address) > 1)
        {
            Remove(flow.OpenAccount(Address));
        }
        // The calendar account a password made is named by the login typed, which is no address,
        // so the list's own lookup does not see it.
        flow.AccountRows();
        var settings = flow.Settings();
        if (app.Agent.Find(new Filter { Type = "Button", Name = SeparateLogin, Under = settings }) is { } linked)
        {
            app.Agent.Invoke(linked);
            if (app.Agent.Wait(new Filter { Id = "AccountRemove", Under = settings }, TimeSpan.FromSeconds(10)) is not null)
            {
                Remove(settings);
            }
        }
        app.CloseDialogs();
        if (flow.AccountRows().Any(row => row.Name == Address))
        {
            flow.RemoveOnlyAccount(Address);
        }
    }

    /// <summary>Presses the sign-in of the form under <paramref name="root"/> once it is pressable,
    /// waits for the line's Cancel, and returns the login token the fixture gave the sign-in.</summary>
    private static string StartSignIn(App app, Element? root, string? before)
    {
        Element? signIn = null;
        Assert.True(App.Eventually(() => (signIn = app.Agent.Find(new Filter { Id = "SeparateServerSignIn", Type = "Button", Under = root })) is { Enabled: true },
            TimeSpan.FromSeconds(10)), "the sign-in is pressable");
        app.Agent.Invoke(signIn!);
        Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Id = "SeparateServerSignInCancel", Type = "Button", Under = root }) is not null,
            TimeSpan.FromSeconds(10)), "a sign-in out draws its Cancel");
        return NextcloudBrowser.NextLoginToken(before);
    }

    /// <summary>Grants <paramref name="token"/> at the fixture, as a person who finished in the browser
    /// after the app let go would, and asserts that <paramref name="unmoved"/> still holds once the
    /// app has had time to collect it: the core polls every two seconds and connects in about as long.</summary>
    private static void AssertTheGrantCompletesNothing(string token, Func<bool> unmoved, string what)
    {
        NextcloudBrowser.GrantToken(NextcloudAccountPageTests.Fixture, token, SeparateLogin, "nextcloud-alice-pw");
        // A wait for something not to happen has no event to poll for.
        Thread.Sleep(TimeSpan.FromSeconds(10));
        Assert.True(App.Eventually(unmoved, TimeSpan.FromSeconds(5)), $"{what} is over: granting it afterwards moves nothing");
    }

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void ASignInCancelledOrSupersededOnTheStepCompletesNothingAndTheFreshOneDoes() =>
        Removing(nameof(ASignInCancelledOrSupersededOnTheStepCompletesNothingAndTheFreshOneDoes), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            bool OnTheStep() => app.Agent.Find(Filter.ById("SeparateServerConnect", "Button")) is not null;

            var cancelled = StartSignIn(app, null, NextcloudBrowser.NewestLoginToken());
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button"))!);
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button")) is null
                && app.Agent.Find(Filter.ById("SeparateServerSignInNote")) is { } note && note.Name == Catalog.English("nextcloud_sign_in_note")
                && app.Agent.Find(Filter.ById("SeparateServerSignIn", "Button")) is { Enabled: true }, TimeSpan.FromSeconds(10)),
                "Cancel ends the sign-in: its Cancel goes, the line is the offer again and the button is free");
            AssertTheGrantCompletesNothing(cancelled, OnTheStep, "a cancelled sign-in");

            var superseded = StartSignIn(app, null, cancelled);
            var fresh = StartSignIn(app, null, superseded);
            AssertTheGrantCompletesNothing(superseded, OnTheStep, "a sign-in a second press replaced");

            NextcloudBrowser.GrantToken(NextcloudAccountPageTests.Fixture, fresh, SeparateLogin, "nextcloud-alice-pw");
            Assert.True(app.Agent.WaitGone(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(60)),
                "the sign-in the second press started completes, and setup goes on");
            flow.CompleteSetupPrompts();
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void ClosingThePagesFormCancelsItsSignInAndThePagesOwnCompletes() =>
        Removing(nameof(ClosingThePagesFormCancelsItsSignInAndThePagesOwnCompletes), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            var cancelled = StartSignIn(app, null, NextcloudBrowser.NewestLoginToken());
            Assert.False(app.Agent.Find(Filter.ById("SeparateServerSecondary", "Button"))?.Enabled ?? true,
                "docs/waiting.md: Skip is held while the sign-in is out");
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button"))!);
            var dialog = SkipThenSwitchTheCalendarBackOn(app, flow);
            // The row offers its own sign-in for as long as the calendar is not on an account of its own.
            bool Offered() => InPage(app, dialog, "AccountUseSeparateSignIn_Calendar") is not null;

            void OpenTheForm()
            {
                app.Agent.Invoke(InPage(app, dialog, "AccountUseSeparateSignIn_Calendar")
                    ?? throw new InvalidOperationException("the calendar's sign-in action went away before it was pressed"));
                Assert.True(app.Agent.Wait(new Filter { Id = "SeparateServerSignIn", Under = dialog }, TimeSpan.FromSeconds(10)) is not null,
                    "the page's form offers Nextcloud's sign-in");
            }
            OpenTheForm();
            var closed = StartSignIn(app, dialog, cancelled);
            Assert.True(InPage(app, dialog, "SeparateServerSecondary") is { Enabled: true }, "the page's Cancel is free while the browser waits");
            app.Agent.Invoke(InPage(app, dialog, "SeparateServerSecondary")!);
            Assert.True(App.Eventually(() => Offered() && InPage(app, dialog, "SeparateServerConnect") is null, TimeSpan.FromSeconds(10)),
                "the page's Cancel closes the form with the sign-in out");
            AssertTheGrantCompletesNothing(closed, Offered, "a sign-in out when the page's form was closed");

            OpenTheForm();
            var finished = StartSignIn(app, dialog, closed);
            NextcloudBrowser.GrantToken(NextcloudAccountPageTests.Fixture, finished, SeparateLogin, "nextcloud-alice-pw");
            Assert.True(app.Agent.WaitGone(new Filter { Id = "SeparateServerConnect", Under = dialog }, TimeSpan.FromSeconds(60)),
                "the page's sign-in moves the calendar to an account of its own, and the form closes");
            app.Agent.Settle();
            Assert.False(App.Eventually(Offered, TimeSpan.FromSeconds(5)),
                "and the action does not come back: nothing waits on a sign-in any more");
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void EscapeOnTheAddAccountDialogEndsTheSignInOutAndKeepsTheStep() =>
        Removing(nameof(EscapeOnTheAddAccountDialogEndsTheSignInOutAndKeepsTheStep), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SeparateServerSecondary", "Button"))!);
            Assert.True(app.Agent.WaitGone(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(30)),
                "Skip leaves the separate-server step");
            flow.CompleteSetupPrompts();

            app.Agent.Invoke(app.Agent.Find(Filter.ById("NavAddAccount", "ListItem"))
                ?? throw new InvalidOperationException("the sidebar has no Add account row (#NavAddAccount)"));
            ConnectToTheSeparateServerStep(app, flow, Bob, "harness-bob-pw");
            Assert.True(app.Agent.Find(Filter.ById("AddAccountDialog")) is not null, "the step is in the add-account dialog");
            bool OnTheStep() => app.Agent.Find(Filter.ById("SeparateServerConnect", "Button")) is not null;

            var dismissed = StartSignIn(app, null, NextcloudBrowser.NewestLoginToken());
            // Escape is what a ContentDialog closes on, and the form's own dismissal answers it. The
            // window comes to the front first: bringing it there moves the keyboard to its title bar.
            // The fields are held while the sign-in is out, so the keyboard goes to its Cancel.
            Input.BringToFront(app.Mailbox.Hwnd);
            app.Agent.Focus(app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button"))!);
            Assert.True(App.Eventually(() => app.Agent.Focused() is { Id: "SeparateServerSignInCancel" }, TimeSpan.FromSeconds(5)),
                "the keyboard is in the dialog before Escape is pressed");
            Input.Press(Input.Escape);
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button")) is null && OnTheStep(),
                TimeSpan.FromSeconds(10)), "Escape with a sign-in out ends the sign-in and leaves the step on screen");
            AssertTheGrantCompletesNothing(dismissed, OnTheStep, "a sign-in out when the dialog was dismissed");

            // Bob's mailbox was added before the step; Skip ends the setup the dialog holds.
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SeparateServerSecondary", "Button")) is { Enabled: true } skip
                ? skip
                : throw new InvalidOperationException("Skip is not free once the sign-in has ended"));
            Assert.True(app.Agent.WaitGone(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(30)),
                "Skip leaves the separate-server step");
            flow.CompleteSetupPrompts();
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void AMailboxWhoseCalendarServerIsNextcloudIsOfferedItsSignInOnTheSeparateServerStep() =>
        Case(nameof(AMailboxWhoseCalendarServerIsNextcloudIsOfferedItsSignInOnTheSeparateServerStep), (app, flow) =>
        {
            ConnectToTheSeparateServerStep(app, flow);
            Element? signIn = null;
            Assert.True(App.Eventually(() => (signIn = app.Agent.Find(Filter.ById("SeparateServerSignIn", "Button"))) is { Width: > 0 }, TimeSpan.FromSeconds(15)),
                "rule 2: a separate server that is Nextcloud offers its own sign-in above the password field");
            Assert.Equal(Catalog.English("nextcloud_sign_in_button"), signIn!.Name);
            Assert.True(signIn.Y < Field(app, "SeparateServerUsername").Y, "above the login and password");

            var before = NextcloudBrowser.NewestLoginToken();
            app.Agent.Invoke(signIn);
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("SeparateServerSignInCancel", "Button")) is { Width: > 0 }, TimeSpan.FromSeconds(10)),
                "the wait draws its Cancel");
            // The core holds the sign-in: it says where the person is, holds Connect while the
            // browser has it, and once the browser has had a moment to come up, which the button
            // spends running, makes it pressable again for a fresh sign-in.
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("SeparateServerSignInNote")) is { } note
                && note.Name == Catalog.English("nextcloud_sign_in_waiting"), TimeSpan.FromSeconds(5)),
                "rule 5: while the browser holds the sign-in, the line says the app notices when the person is done");
            Assert.False(app.Agent.Find(Filter.ById("SeparateServerConnect", "Button"))?.Enabled ?? true,
                "Connect is held while the sign-in is out");
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("SeparateServerSignIn", "Button")) is { Enabled: true },
                TimeSpan.FromSeconds(10)),
                "the sign-in is pressable again while the browser waits, so a press starts it afresh");
            NextcloudBrowser.GrantNext(NextcloudAccountPageTests.Fixture, before, "alice", "nextcloud-alice-pw");
            Assert.True(app.Agent.WaitGone(Filter.ById("SeparateServerConnect", "Button"), TimeSpan.FromSeconds(60)),
                "the grant moves the calendar and contacts to an account of their own, and setup goes on");
            flow.CompleteSetupPrompts();
            Assert.True(App.Eventually(() => flow.AccountRows().Count(row => row.Name == Address) == 2, TimeSpan.FromSeconds(15)),
                "the mailbox and the calendar and contacts account beside it");
        });

    [NextcloudFixtureFact(NeedsHarness = true)]
    public void RemovingBothReturnsTheWindowToFirstRunSetup() => Case(nameof(RemovingBothReturnsTheWindowToFirstRunSetup), (app, flow) =>
    {
        while (flow.AccountRows().Count(row => row.Name == Address) > 1)
        {
            var dialog = flow.OpenAccount(Address);
            app.Agent.Invoke(app.Agent.Find(new Filter { Id = "AccountRemove", Under = dialog })!);
            app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "AccountRemoveConfirm", Under = dialog }, TimeSpan.FromSeconds(10))!);
            app.Agent.Settle();
        }
        app.CloseDialogs();
        flow.RemoveOnlyAccount(Address);
    });
}
