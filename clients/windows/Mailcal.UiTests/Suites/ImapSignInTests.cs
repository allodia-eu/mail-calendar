// Adding a mail account whose server may offer a sign-in: what the setup form asks for, as the
// server answers it (docs/mail-oauth.md rules 2, 3 and 8).
//
// WHY HERE. ImapSignInGate decides what belongs on screen and is unit-tested in Mailcal.Tests, but
// what IS on screen is the view reading it, and a panel left visible, a field toggled on the wrong
// element or a route that skips the refresh all render without an error. Every answer comes from a
// real server (docker/stalwart/README.md, "Signing in to an IMAP account"):
//
//   alice@localhost, detected                  sign in, and a password also works
//   alice@test.local at localhost:12993        only pre-registered apps, with the password field
//   a listener that accepts and says nothing   the password field, once the core gives up
//
// The form is opened from the sidebar in a dialog over the running app and closed again by every
// case, and nothing here connects: Connect is never pressed. The sign-in itself is pressed once,
// with the harness's HTTPS front stopped so discovery fails before any browser opens.
//
// KNOWN GAP. Cancel while a sign-in is out is not asserted: reaching that state opens a real browser
// at the sign-in server. Verify it by hand: add alice@localhost, press "Sign in with your provider",
// then Cancel, and the form stays with the button enabled and no warning.

using System.Net;
using System.Net.Sockets;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class ImapSignInTests(HarnessApp harness)
{
    // Written as lookups so check-automation-ids reads them.
    private static readonly string[] SetupIds = new[]
    {
        Filter.ById("ImapCheckingText"), Filter.ById("ImapSignInButton"), Filter.ById("ImapUsePasswordButton"),
        Filter.ById("ImapRegistrationNeededNote"), Filter.ById("ImapSignInFailedBar"), Filter.ById("Password"), Filter.ById("ConnectButton"),
    }.Select(filter => filter.Id!).ToArray();

    private void Case(string name, Action<App> body, Action<App>? cleanup = null) =>
        harness.Case(name, body, app =>
        {
            try
            {
                cleanup?.Invoke(app);
            }
            finally
            {
                CloseSetup(app);
            }
        });

    private static Element OpenSetup(App app)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavAddAccount", "ListItem"))
            ?? throw new InvalidOperationException("the sidebar has no Add account row (#NavAddAccount) to open the form with"));
        return app.Agent.Wait(Filter.ById("DetectEmail", "Edit"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("pressing Add account opened no setup form (#DetectEmail)");
    }

    private static void CloseSetup(App app)
    {
        foreach (var id in new[] { "DetectCancelButton", "CancelButton" })
        {
            if (app.Agent.Find(Filter.ById(id, "Button")) is { Enabled: true } cancel)
            {
                app.Agent.Invoke(cancel);
                break;
            }
        }
        app.Agent.WaitGone(Filter.ById("AddAccountDialog"), TimeSpan.FromSeconds(5));
        // And its dimming layer, which outlives it and swallows the next Add account.
        app.CloseDialogs();
    }

    /// <summary>
    /// The form's sign-in controls drawn right now, by id. A collapsed control has no peer, so
    /// presence is the test, not being on screen: the form scrolls inside the dialog, and a field
    /// below the fold is drawn and out of view at once. The failure bar is the exception: an InfoBar
    /// keeps its peer while closed and drops its message, so it counts only while it holds one.
    /// </summary>
    private static Dictionary<string, Element> Shown(App app)
    {
        var dialog = app.Agent.Find(Filter.ById("AddAccountDialog")) ?? throw new InvalidOperationException("the add-account dialog is not on screen");
        var shown = app.Agent.FindAll(new Filter { Under = dialog })
            .Where(e => SetupIds.Contains(e.Id))
            .GroupBy(e => e.Id)
            .ToDictionary(g => g.Key, g => g.First());
        if (shown.Remove("ImapSignInFailedBar", out var bar)
            && app.Agent.FindAll(new Filter { Type = "Text", Under = bar }).Any(text => text.Name.Length > 20))
        {
            shown["ImapSignInFailedBar"] = bar;
        }
        return shown;
    }

    /// <summary>
    /// Polls the form until <paramref name="until"/> holds and returns every state it passed through.
    /// The record is the point: "no password field before the answer" and "the field never leaves
    /// while the server is asked again" are claims about every frame, not about where the form ends.
    /// </summary>
    private static List<Dictionary<string, Element>> Watch(App app, string what, Func<Dictionary<string, Element>, bool> until, int timeoutSec = 25)
    {
        var states = new List<Dictionary<string, Element>>();
        var deadline = DateTime.UtcNow.AddSeconds(timeoutSec);
        while (DateTime.UtcNow < deadline)
        {
            states.Add(Shown(app));
            if (until(states[^1]))
            {
                return states;
            }
            Thread.Sleep(100);
        }
        throw new InvalidOperationException($"the form never reached {what} within {timeoutSec} s; last drawn: [{string.Join(", ", states[^1].Keys.Order())}]");
    }

    private static void SetField(App app, string id, string text) =>
        app.Agent.SetValue(app.Agent.Find(Filter.ById(id, "Edit")) ?? throw new InvalidOperationException($"the form has no #{id} to type into"), text);

    private static string Field(App app, string id) => app.Agent.Read(app.Agent.Find(Filter.ById(id, "Edit"))!).Value ?? "";

    private static void OpenDetectedCard(App app, string address)
    {
        app.Agent.SetValue(OpenSetup(app), address);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ContinueButton", "Button"))!);
        if (app.Agent.Wait(Filter.ById("SetupMailServerIMAP", "Text"), TimeSpan.FromSeconds(30)) is null)
        {
            throw new InvalidOperationException($"detection for {address} never reached the account form");
        }
    }

    private static void OpenManualForm(App app, string address, int port)
    {
        app.Agent.SetValue(OpenSetup(app), address);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ManualButton", "Button"))!);
        if (app.Agent.Wait(Filter.ById("ImapHost", "Edit"), TimeSpan.FromSeconds(10)) is null)
        {
            throw new InvalidOperationException("\"Set up manually\" opened no manual form");
        }
        SetField(app, "ImapPort", port.ToString(System.Globalization.CultureInfo.InvariantCulture));
        SetField(app, "ImapHost", "localhost");
    }

    /// <summary>Chooses <paramref name="label"/> in the IMAP security picker. Its labels are protocol
    /// names, the same in every catalog locale.</summary>
    private static void SelectSecurity(App app, string label)
    {
        var picker = app.Agent.Find(Filter.ById("ImapSecurityPicker", "ComboBox")) ?? throw new InvalidOperationException("the manual form has no IMAP security picker");
        app.Agent.Expand(picker);
        // The combo also lists its items under itself; the open list's own are the ones drawn, and
        // neither takes a selection through its peer, so the choice is a click, as a person makes it.
        var named = Filter.Named(label, "ListItem");
        Element? item = null;
        if (!App.Eventually(() =>
            {
                var closed = app.Agent.FindAll(named with { Under = picker }).Select(e => e.Handle).ToHashSet();
                return (item = app.Agent.FindAll(named).FirstOrDefault(e => !closed.Contains(e.Handle))) is not null;
            }, TimeSpan.FromSeconds(3)))
        {
            throw new InvalidOperationException($"the IMAP security picker offers no '{label}'");
        }
        Input.Click(app.Steady(item!));
        App.Eventually(() => app.Agent.FindAll(named).Count <= app.Agent.FindAll(named with { Under = picker }).Count, TimeSpan.FromSeconds(3));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
    }

    [HarnessFact]
    public void AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl() =>
        Case(nameof(AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl), app =>
        {
            OpenDetectedCard(app, "alice@localhost");
            var states = Watch(app, "the sign-in offer", s => s.ContainsKey("ImapSignInButton"));
            var early = states.FindIndex(s => s.ContainsKey("Password") || s.ContainsKey("ConnectButton"));
            Assert.True(early < 0,
                $"no password field and no Connect may be drawn before, or beside, a sign-in the person has not turned down (docs/mail-oauth.md rules 2 and 8); frame {early} of {states.Count} drew [{(early < 0 ? "" : string.Join(", ", states[early].Keys.Order()))}]");
            Assert.True(states[^1].TryGetValue("ImapUsePasswordButton", out var usePassword),
                "a server that also takes a password offers \"Use a password instead\" under the sign-in");
            app.Agent.Invoke(usePassword!);
            var chosen = Watch(app, "the password route", s => s.ContainsKey("Password"), timeoutSec: 5)[^1];
            Assert.True(chosen.ContainsKey("ConnectButton"), "choosing the password route draws the field and Connect");
            Assert.True(chosen.ContainsKey("ImapSignInButton"), "the sign-in stays on screen as an ordinary button once a password is chosen");
            Assert.False(chosen.ContainsKey("ImapUsePasswordButton"), "the control that drew the field has nothing left to do");
        });

    [HarnessFact]
    public void ThePortFieldsShowWhatTheFormDialsNeverTheLastAccountsAndThePickerMovesThem() =>
        Case(nameof(ThePortFieldsShowWhatTheFormDialsNeverTheLastAccountsAndThePickerMovesThem), app =>
        {
            OpenDetectedCard(app, "alice@localhost");
            Watch(app, "the sign-in offer", s => s.ContainsKey("ImapSignInButton"));
            Assert.True(app.Agent.Find(Filter.ById("SetupMailServerIMAP", "Text"))?.Name.Contains(":12995 ", StringComparison.Ordinal) == true,
                "the harness autoconfig names IMAP on 12995, which is what this case needs to have on screen first");
            app.Agent.Invoke(app.Agent.Find(Filter.ById("BackButton", "Button"))!);
            app.Agent.Invoke(app.Agent.Wait(Filter.ById("ManualButton", "Button"), TimeSpan.FromSeconds(5))!);
            app.Agent.Wait(Filter.ById("ImapPort", "Edit"), TimeSpan.FromSeconds(5));
            Assert.True(Field(app, "ImapPort") == "993", "the manual form shows the standard IMAP port, the one it dials, not the last account's");
            Assert.True(Field(app, "SmtpPort") == "465", "the manual form shows the standard submission port, not the last account's");
            // Writing those ports is the app's own fill, not the person typing one, so the port
            // still follows the picker (docs/account-autodetect.md).
            SelectSecurity(app, "STARTTLS");
            Assert.True(Field(app, "ImapPort") == "143", "on a fresh manual form the IMAP port follows the security picker until somebody types one");
            SelectSecurity(app, "SSL/TLS");
            Assert.True(Field(app, "ImapPort") == "993", "and follows it back");
            SetField(app, "ImapPort", "1993");
            SelectSecurity(app, "STARTTLS");
            Assert.True(Field(app, "ImapPort") == "1993", "a port the person typed is theirs, and the picker leaves it alone");
        });

    [HarnessFact]
    public void AServerAdmittingOnlyPreRegisteredAppsSaysSoAndDrawsThePasswordField() =>
        Case(nameof(AServerAdmittingOnlyPreRegisteredAppsSaysSoAndDrawsThePasswordField), app =>
        {
            OpenManualForm(app, Harness.Alice, 12993);
            var states = Watch(app, "the closed-registration line", s => s.ContainsKey("ImapRegistrationNeededNote"));
            Assert.False(states.Any(s => s.ContainsKey("Password") && !s.ContainsKey("ImapRegistrationNeededNote")), "the password field waits for the first answer");
            var answer = states[^1];
            Assert.True(answer.ContainsKey("Password") && answer.ContainsKey("ConnectButton"), "a provider whose sign-in is closed to this app still takes a password here");
            Assert.False(answer.ContainsKey("ImapSignInButton"), "there is no sign-in this app can start");

            // Rule 8: the field stays while the edited account is asked about again. The app marks the
            // question as it goes out, which proves a second one was asked rather than the first
            // answer kept: the line it clears can be back before any look at the form sees it go.
            SetField(app, "Username", Harness.Bob);
            var again = Watch(app, "a second answer", s => s.ContainsKey("ImapRegistrationNeededNote")
                && app.Agent.Find(new Filter { Id = "Username", Signal = $"asked about {Harness.Bob}" }) is not null);
            Assert.False(again.Any(s => !s.ContainsKey("Password") || !s.ContainsKey("ConnectButton")),
                "a password field the form has shown stays, with Connect, while the server is asked again");
        });

    [HarnessFact]
    public void AServerThatNeverAnswersLeavesThePasswordField()
    {
        // A port that completes the TCP handshake and then never says a word, which is what an
        // overloaded server or a stalling middlebox looks like to the probe. Nothing accepts: the
        // listen backlog is enough for the client's connect to succeed.
        var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        Case(nameof(AServerThatNeverAnswersLeavesThePasswordField), app =>
        {
            OpenManualForm(app, Harness.Alice, ((IPEndPoint)listener.LocalEndpoint).Port);
            var states = Watch(app, "the password field", s => s.ContainsKey("Password"), timeoutSec: 20);
            Assert.True(states.Any(s => s.ContainsKey("ImapCheckingText")), "while the server is asked the form says so, rather than showing nothing");
            var answer = states[^1];
            Assert.True(answer.ContainsKey("ConnectButton"), "Connect comes with the field");
            Assert.False(answer.ContainsKey("ImapSignInButton") || answer.ContainsKey("ImapCheckingText"), "an unanswered question is a password form and nothing else (rule 7)");
        }, _ => listener.Stop());
    }

    [HarnessFact]
    public void AFailedSignInSaysSoAndHandsThePasswordFieldBack() =>
        Case(nameof(AFailedSignInSaysSoAndHandsThePasswordFieldBack), app =>
        {
            OpenDetectedCard(app, "alice@localhost");
            var offer = Watch(app, "the sign-in offer", s => s.ContainsKey("ImapSignInButton"))[^1];
            // Discovery goes through the HTTPS front, so with it stopped the sign-in fails before a
            // browser is opened.
            Harness.SignInFront("stop");
            app.Agent.Invoke(offer["ImapSignInButton"]);
            Watch(app, "the failure warning", s => s.ContainsKey("ImapSignInFailedBar"));
            Harness.SignInFront("start");
            // The route comes back on the frame after the warning.
            var failed = Shown(app);
            App.Eventually(() => (failed = Shown(app)).ContainsKey("Password") && failed.ContainsKey("ConnectButton"), TimeSpan.FromSeconds(5));
            Assert.True(failed.ContainsKey("Password") && failed.ContainsKey("ConnectButton"), "a sign-in that did not work brings the password route back, whatever the server said");
            Assert.True(failed.TryGetValue("ImapSignInButton", out var again) && app.Agent.Read(again).Element.Enabled, "the sign-in can be tried again");
        }, _ => Harness.SignInFront("start"));
}
