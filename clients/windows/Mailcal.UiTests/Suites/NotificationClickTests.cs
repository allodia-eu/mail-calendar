// A click on a new-mail notification opens the message it names, with the app running and with the
// app quit (docs/background-sync.md, Enforcement).
//
// THE CLICK IS THE SHELL'S. The suite makes the COM call the shell makes on a press, with the launch
// arguments read back from the shell's notification history, so what is clicked is what the app
// posted (Mailcal.Tests pins the encoding), and this is also the gate that a notification reaches
// the shell at all. A running app answers from the class object it registered, which the Windows App
// SDK raises as NotificationInvoked; with the app quit, COM starts the exe from the activator's
// LocalServer32 command line and the click arrives as the launch's own activation.
//
// The cold leg reaches the harness because COM starts a local server with the activating process's
// environment; a click by hand is activated by the shell, whose environment opens the ordinary store
// (docs/debugging.md). On IMAP, because IDLE delivers into the running app, and a notification is
// raised only for mail that arrives while it is open.
//
// A host that cannot show notifications registers and posts nothing, and fails here rather than
// skipping: the harness suites run on a developer's desktop, and xUnit decides a skip before the app
// has said whether it can. The cold leg quits this suite's launch, so it comes last.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
[Trait("Dataset", "harness")]
public sealed class NotificationClickTests(NotificationHarnessApp harness) : IClassFixture<NotificationHarnessApp>
{
    // A delivery reaches the running app over IDLE and is then announced; both are well inside this.
    private static readonly TimeSpan PostedWithin = TimeSpan.FromSeconds(60);

    // The cold leg starts the app, connects the account and opens the message from the store.
    private static readonly TimeSpan OpenedWithin = TimeSpan.FromSeconds(90);

    /// <summary>Appends a message to alice's Inbox, as harness.sh deliver does, and returns the
    /// registration and the arguments of the toast the app posted for it.</summary>
    private static ((string Aumid, Guid Clsid) Registration, string Arguments) Deliver(App app, string subject)
    {
        var registered = AppLog.NewestSession().Split('\n').LastOrDefault(line => line.Contains("notifications: registered", StringComparison.Ordinal));
        if (registered?.Contains("system setting Unsupported", StringComparison.Ordinal) == true)
        {
            throw new InvalidOperationException($"this host cannot show notifications, so there is no toast to click (the app logged: {registered.Trim()})");
        }
        var registration = Notifications.Registration(System.Diagnostics.Process.GetProcessById(app.ProcessId).MainModule!.FileName);
        Harness.ImportAsync(string.Join("\r\n",
            $"From: {Harness.Bob}", $"To: {Harness.Alice}", $"Subject: {subject}", $"Date: {DateTime.UtcNow:r}",
            "", "Appended by the notification suite.", ""), "inbox").GetAwaiter().GetResult();
        return (registration, Notifications.PostedToast(registration.Aumid, subject, PostedWithin));
    }

    private static bool WaitReadingSubject(App app, string subject, TimeSpan timeout) =>
        app.Agent.Wait(Filter.ById("ReadingSubject") with { Name = subject, OnScreen = true }, timeout) is not null;

    [HarnessFact]
    public void AClickWithTheAppRunningOpensTheMessageFromTheCalendar()
    {
        var subject = $"Windows notification click {Guid.NewGuid():N}";
        harness.Case(nameof(AClickWithTheAppRunningOpensTheMessageFromTheCalendar), app =>
        {
            var (registration, arguments) = Deliver(app, subject);
            // Over the calendar, because a click takes the app to mail before it opens anything,
            // and there neither the row nor the heading is on screen.
            new CalendarFlow(app).ShowCalendar();
            Notifications.Activate(registration, arguments);
            Assert.True(WaitReadingSubject(app, subject, App.Patience),
                $"a click on the notification for '{subject}' brings the mailbox back and opens that message (docs/background-sync.md)");
        }, _ => Harness.RemoveMail(Harness.Alice, subject));
    }

    [HarnessFact]
    public void AClickOverAComposerKeepsWhatWasWrittenInDraftsAndOpensTheMessage()
    {
        var subject = $"Windows notification click over a draft {Guid.NewGuid():N}";
        var draft = $"Notification suite draft {Guid.NewGuid():N}";
        harness.Case(nameof(AClickOverAComposerKeepsWhatWasWrittenInDraftsAndOpensTheMessage), app =>
        {
            var drafts = new DraftsFlow(app);
            var (registration, arguments) = Deliver(app, subject);
            drafts.OpenNewComposer();
            drafts.SetText("SubjectBox", draft);
            Notifications.Activate(registration, arguments);
            Assert.True(WaitReadingSubject(app, subject, App.Patience), $"a click over a composer opens the message it names, '{subject}', in the composer's place");
            Assert.True(app.Agent.Find(Filter.ById("PrimaryButton", "Button") with { OnScreen = true }) is null,
                "a click over a composer asked a question; leaving a composer keeps the draft unasked (docs/drafts.md)");
            Assert.True(drafts.WaitDraftRow(draft), $"the composer a click replaced is not in Drafts as '{draft}': leaving saves what was written");
        }, app =>
        {
            new DraftsFlow(app).ClearStoredDraft(draft);
            Harness.RemoveMail(Harness.Alice, subject);
        });
    }

    [HarnessFact]
    public void AClickThatStartsTheAppOpensTheMessage()
    {
        var subject = $"Windows notification cold click {Guid.NewGuid():N}";
        App? started = null;
        harness.Case(nameof(AClickThatStartsTheAppOpensTheMessage), app =>
        {
            var (registration, arguments) = Deliver(app, subject);
            // Closed rather than killed: the window's Closed handler unregisters the activator's class
            // object, and only then does COM start a new process for the click.
            if (!app.CloseMailbox(TimeSpan.FromSeconds(30)))
            {
                throw new InvalidOperationException("the app was still running 30 s after its window was closed, so the click would reach it warm");
            }
            started = App.StartedBy(() => Notifications.Activate(registration, arguments), NotificationHarnessApp.ImapEnv);
            var command = Notifications.CommandLine(started.ProcessId);
            Assert.True(command.Contains("----AppNotificationActivated:", StringComparison.Ordinal),
                $"the click starts the app through its notification activator; the process running is: {command}");
            Assert.True(WaitReadingSubject(started, subject, OpenedWithin),
                $"a click that starts the app opens the message it names, '{subject}', once the shell is up (Services/NotificationOpenInbox.cs)");
        }, _ =>
        {
            started?.Dispose();
            Harness.RemoveMail(Harness.Alice, subject);
        });
    }
}
