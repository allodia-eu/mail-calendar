// The moves the suites that set an account up share: the manual form from the first-run screen, the
// questions setting an account up asks afterwards, an account's page in Settings → Accounts, and
// removing the account again so the next suite starts where this one did.

using Allodia.Mailcal.UiTests.Driver;

namespace Allodia.Mailcal.UiTests.Suites;

internal sealed class SetupFlow(App app)
{
    private Agent Agent => app.Agent;

    /// <summary>
    /// From the first-run screen, the manual form with <paramref name="type"/> chosen. "Set up
    /// manually" rather than Continue: detection for test.local reaches no server, and waiting it out
    /// would test the strategies' timeouts rather than the form.
    /// </summary>
    internal void OpenManualSetup(string address, Filter type)
    {
        var email = Agent.Wait(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }, TimeSpan.FromSeconds(15))
            ?? throw new InvalidOperationException("the first-run screen has no address field (#DetectEmail)");
        Agent.SetValue(email, address);
        Agent.Invoke(Agent.Find(Filter.ById("ManualButton", "Button")) ?? throw new InvalidOperationException("the first-run screen offers no manual set-up"));
        var choice = Agent.Wait(type, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException($"the manual form has no #{type.Id} account type");
        Agent.Select(choice);
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
    }

    /// <summary>
    /// Answers what setting an account up asks once it has connected, each with the answer that
    /// changes nothing, until the mailbox is on screen with none of them up: the link step, skipped;
    /// the name a mailbox sends under, skipped; and the default-mail-app offer, declined. Which of
    /// them a route asks is the route's business and asserted elsewhere, so each is answered only if
    /// it comes. The offer comes last when it comes, so answering it ends the wait; otherwise the
    /// mailbox has to stay quiet for three seconds, because one question can follow another.
    /// </summary>
    internal void CompleteSetupPrompts()
    {
        var deadline = DateTime.UtcNow.AddSeconds(60);
        DateTime? quietSince = null;
        while (DateTime.UtcNow < deadline)
        {
            if (Agent.Find(Filter.ById("SetupLinksSkip") with { OnScreen = true }) is { } skip)
            {
                Agent.Invoke(skip);
                Agent.Settle();
                quietSince = null;
                continue;
            }
            var offer = Agent.Find(Filter.ById("DefaultMailAppOffer"));
            if (offer is not null || Agent.Find(Filter.ById("SenderNamePrompt")) is not null)
            {
                app.CloseDialogs();
                if (offer is not null)
                {
                    return;
                }
                quietSince = null;
                continue;
            }
            if (Agent.Find(Filter.ById("SettingsItem") with { OnScreen = true }) is null || Agent.Find(Filter.ById("ConnectButton") with { OnScreen = true }) is not null)
            {
                quietSince = null;
            }
            else if ((quietSince ??= DateTime.UtcNow) < DateTime.UtcNow.AddSeconds(-3))
            {
                return;
            }
            Thread.Sleep(100);
        }
        throw new InvalidOperationException("setting the account up did not reach the mailbox within 60 s");
    }

    /// <summary>The Settings dialog, opened from the sidebar gear if it is not up already.</summary>
    internal Element Settings()
    {
        var holding = Filter.ById("SettingsCategory_accounts");
        if (Agent.Find(new Filter { Type = "Window", Window = app.Mailbox.Hwnd, Has = holding }) is { } open)
        {
            return open;
        }
        Agent.Invoke(Agent.Find(Filter.ById("SettingsItem")) ?? throw new InvalidOperationException("the sidebar has no Settings gear (#SettingsItem)"));
        return app.Dialog(holding);
    }

    /// <summary>
    /// The account rows of Settings → Accounts, drawn afresh, by leaving the category and coming
    /// back, so what they show is the core's answer and not a value the client kept; and selecting
    /// Accounts while an account's page is open leaves that page on screen.
    /// </summary>
    internal IReadOnlyList<Element> AccountRows()
    {
        var dialog = Settings();
        Agent.Select(Agent.Find(new Filter { Id = "SettingsCategory_general", Under = dialog })!);
        Agent.Select(Agent.Find(new Filter { Id = "SettingsCategory_accounts", Under = dialog })!);
        Agent.Wait(new Filter { Type = "Button", NameLike = "*@*", Under = dialog }, TimeSpan.FromSeconds(10));
        return Agent.FindAll(new Filter { Type = "Button", NameLike = "*@*", Under = dialog });
    }

    /// <summary>
    /// Opens <paramref name="address"/>'s page in Settings → Accounts and returns the dialog: the
    /// <paramref name="index"/>th account of that address, as the list orders them, since one address
    /// can be a mailbox and a calendar account apart.
    /// </summary>
    internal Element OpenAccount(string address, int index = 0)
    {
        List<Element> rows = [];
        if (!App.Eventually(() => (rows = AccountRows().Where(r => r.Name == address).ToList()).Count > index, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException($"Settings → Accounts lists {rows.Count} account(s) for {address}, not {index + 1}");
        }
        var row = rows[index];
        var dialog = Settings();
        Agent.Invoke(row);
        if (Agent.Wait(new Filter { Id = "AccountRemove", Under = dialog }, TimeSpan.FromSeconds(10)) is null)
        {
            throw new InvalidOperationException($"pressing {address} in Settings → Accounts opened no account page");
        }
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        return dialog;
    }

    /// <summary>Removes the one account from its page, and waits for the first-run screen it leaves.</summary>
    internal void RemoveOnlyAccount(string address)
    {
        var dialog = OpenAccount(address);
        Agent.Invoke(Agent.Find(new Filter { Id = "AccountRemove", Under = dialog })!);
        Agent.Invoke(Agent.Wait(new Filter { Id = "AccountRemoveConfirm", Under = dialog }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Remove asked nothing before removing the account"));
        app.CloseDialogs();
        if (Agent.Wait(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }, TimeSpan.FromSeconds(20)) is null)
        {
            throw new InvalidOperationException("removing the last account did not return the window to first-run setup");
        }
    }
}
