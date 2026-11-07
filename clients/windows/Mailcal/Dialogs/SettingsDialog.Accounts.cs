// The Accounts category: every account, mail or not, from the core's one snapshot, each opening a
// page of its own (docs/accounts.md rule 11, docs/settings.md category 9). The page's sections are
// partials of their own: what the account is used for and linked to (AccountUses), the mailboxes
// shared with it (AccountShared), how it signs in (AccountSignIn), its servers and its removal
// (AccountServers), and its mail settings (AccountMail).
//
// State lives in the core. A use, a link and a removal are dispatched and the page says they run
// (docs/waiting.md); every other change forwards to the core off the UI thread. The page is drawn
// again from the next snapshot and page record; a core signal that changed nothing the page shows
// leaves it, and whatever is being typed on it, alone.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The account whose page is open; null for the list.
    private string? _accountPage;

    // What the list or the page last said, kept across the redraw a change ends with.
    private AccountNotice? _accountNotice;
    private InfoBar? _accountNoticeBar;

    // What the open list or page was drawn from, so a signal that changed none of it is ignored.
    private string? _accountsDrawn;

    // Whether there was an account when Settings last looked: the last one going is what closes it.
    private bool _hadAccounts;

    // The confirmation open on the page, so the page drawn again from a core signal draws it open
    // too: a reconnecting account signals throughout, and a question that closed by itself would
    // read as answered.
    private string? _confirmOpen;

    private sealed record AccountNotice(InfoBarSeverity Severity, string Title, string Message);

    private UIElement BuildAccounts()
    {
        var snapshot = _model.AccountsSnapshot();
        var page = OpenPage();
        _accountsDrawn = AccountsFingerprint(snapshot, page);
        var panel = new StackPanel { Spacing = 16 };
        _accountNoticeBar = new InfoBar { IsClosable = true };
        ShowNotice(_accountNoticeBar, _accountNotice);
        _accountNoticeBar.Closed += (_, _) => _accountNotice = null;
        var entry = snapshot.Accounts.FirstOrDefault(account => account.Id == _accountPage);
        if (entry is not null && page is not null)
        {
            panel.Children.Add(BackToAccounts());
            panel.Children.Add(_accountNoticeBar);
            BuildAccountPage(panel, entry, page);
            return panel;
        }
        _accountPage = null;
        panel.Children.Add(_accountNoticeBar);
        var add = new Button { Content = L10n.ActionAddAccount(), Style = AccentButton() };
        add.Click += (_, _) =>
        {
            Hide();
            _model.BeginAddAccount();
        };
        panel.Children.Add(add);
        // What the person's other devices have to say, above their own accounts: an offer becomes
        // one of them.
        if (BuildAllodiaSync() is { } sync)
        {
            panel.Children.Add(sync);
        }
        if (snapshot.Accounts.Length == 0)
        {
            panel.Children.Add(Description(L10n.SettingsAccountsEmpty()));
            return panel;
        }
        var list = new StackPanel { Spacing = 4 };
        foreach (var account in snapshot.Accounts)
        {
            list.Children.Add(AccountRow(account));
        }
        panel.Children.Add(list);
        return panel;
    }

    /// <summary>One account in the list: its address, then its kind, uses and links, and an arrow
    /// to its page.</summary>
    private Button AccountRow(AccountEntry entry)
    {
        var text = new StackPanel { Spacing = 2 };
        text.Children.Add(Heading(entry.Address));
        text.Children.Add(Description(AccountSummary(entry)));
        var row = new Grid { ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(text);
        var arrow = new FontIcon { Glyph = "", FontSize = 12, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(arrow, 1);
        row.Children.Add(arrow);
        var button = new Button
        {
            Content = row,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(12, 8, 12, 8),
        };
        AutomationProperties.SetName(button, entry.Address);
        AutomationProperties.SetHelpText(button, AccountSummary(entry));
        button.Click += (_, _) => OpenAccountPage(entry.Id);
        return button;
    }

    /// <summary>The list row's second line: the kind, what the account is used for, what it is
    /// linked to, and whether something needs the person.</summary>
    private static string AccountSummary(AccountEntry entry)
    {
        var uses = string.Join(", ", AccountSettingsRules.InUse(entry).Select(UseName));
        var lines = new List<string> { $"{AccountNameText.KindLine(entry)} · {uses}" };
        lines.AddRange(AccountSettingsRules.Linked(entry)
            .Select(link => L10n.SettingsAccountLinkedLine(UseName(link.Use), AccountNameText.Of(link.Account))));
        if (AccountSettingsRules.NeedsPermission(entry))
        {
            lines.Add(L10n.SettingsAccountNeedsPermission());
        }
        return string.Join("\n", lines);
    }

    /// <summary>A use's name, as the main window's sections name it.</summary>
    internal static string UseName(AccountCapability capability) => capability switch
    {
        AccountCapability.Calendar => L10n.NavCalendar(),
        AccountCapability.Contacts => L10n.NavContacts(),
        AccountCapability.Colleagues => L10n.AccountUseColleagues(),
        AccountCapability.SharedMailboxes => L10n.AccountUseSharedMailboxes(),
        AccountCapability.Meetings => L10n.AccountUseMeetings(),
        _ => L10n.NavMail(),
    };

    /// <summary>One account's page: how it travels, what it is used for, the accounts it relies
    /// on, the mailboxes shared with it, how it signs in, its own servers, its mail settings while
    /// it is used for mail, and its removal, last.</summary>
    private void BuildAccountPage(StackPanel panel, AccountEntry entry, AccountPage page)
    {
        var title = new TextBlock
        {
            Text = entry.Address,
            Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        panel.Children.Add(title);
        panel.Children.Add(Description(AccountNameText.KindLine(entry)));
        AddAccountNotices(panel, entry.Id);
        // Whether this one travels: first, because it decides whether anything below it is
        // anybody else's business (docs/settings.md).
        if (_model.AccountsSyncMode.TryGetValue(entry.Id, out var mode))
        {
            panel.Children.Add(BuildSyncModePicker(entry.Id, mode));
        }
        panel.Children.Add(UsesGroup(entry, page));
        if (page.Links.Length > 0)
        {
            panel.Children.Add(LinksGroup(entry, page.Links));
        }
        panel.Children.Add(SharedMailboxesSlot(entry));
        var expired = _model.SignInExpiredIds.Contains(entry.Id);
        if (AccountSettingsRules.SignsInAtProvider(entry.Kind))
        {
            panel.Children.Add(SignInGroup(entry, expired));
        }
        else if (AccountSettingsRules.MayHaveSignInSection(entry))
        {
            panel.Children.Add(AccountSignInGroup(entry));
        }
        if (entry.MailSync is { } technology)
        {
            panel.Children.Add(SyncTechnologyGroup(technology));
        }
        if (entry.Endpoints is { } endpoints)
        {
            panel.Children.Add(ServersGroup(entry, endpoints, expired));
        }
        if (MailGroup(entry, expired) is { } mail)
        {
            panel.Children.Add(mail);
        }
        panel.Children.Add(RemoveGroup(entry, page.Remove));
    }

    /// <summary>The open account's page as the core decided it; null for the list, and once the
    /// account is gone.</summary>
    private AccountPage? OpenPage() => _accountPage is { } id ? _model.AccountPageOf(id) : null;

    private Button BackToAccounts()
    {
        var back = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 14 },
            Padding = new Thickness(8),
        };
        AutomationProperties.SetName(back, L10n.A11yBack());
        ToolTipService.SetToolTip(back, L10n.A11yBack());
        back.Click += (_, _) => OpenAccountPage(null);
        return back;
    }

    private void OpenAccountPage(string? accountId)
    {
        _accountPage = accountId;
        _accountNotice = null;
        _confirmOpen = null;
        // A page opened again asks its server again: what it takes, and which mailboxes it shares
        // with the account, can have changed meanwhile.
        _signInOptions.Clear();
        _sharedAnswers.Clear();
        ShowCategory("accounts");
    }

    /// <summary>Says <paramref name="notice"/> and draws the page again from the core.</summary>
    private void NotifyAccounts(AccountNotice? notice)
    {
        _accountNotice = notice;
        if (CurrentCategory() == "accounts")
        {
            ShowCategory("accounts");
        }
    }

    /// <summary>Says <paramref name="notice"/> and leaves the page as it is: for progress, and for
    /// a refusal of what the person typed, which they will want to correct rather than retype.</summary>
    private void SayOnAccounts(AccountNotice? notice)
    {
        _accountNotice = notice;
        if (_accountNoticeBar is { } bar)
        {
            ShowNotice(bar, notice);
        }
    }

    /// <summary>Fills <paramref name="bar"/> with <paramref name="notice"/>, and scrolls it into
    /// view: what it answers was usually pressed further down the page.</summary>
    private static void ShowNotice(InfoBar bar, AccountNotice? notice)
    {
        bar.IsOpen = notice is not null;
        if (notice is null)
        {
            return;
        }
        bar.Severity = notice.Severity;
        bar.Title = notice.Title;
        bar.Message = notice.Message;
        if (bar.IsLoaded)
        {
            bar.StartBringIntoView();
        }
        else
        {
            bar.Loaded += (_, _) => bar.StartBringIntoView();
        }
    }

    /// <summary>A change the page could not make, and why.</summary>
    private static AccountNotice ChangeFailed(string detail) =>
        new(InfoBarSeverity.Error, L10n.SettingsAccountChangeFailedTitle(), detail);

    /// <summary>What a change on the page came to, in the core's words for why.</summary>
    private static AccountNotice ChangeFailed(AccountChangeNotice notice) => notice switch
    {
        AccountChangeNotice.UseFailed use => ChangeFailed(use.Detail),
        AccountChangeNotice.LinkFailed link => ChangeFailed(link.Detail),
        AccountChangeNotice.RemoveFailed removal => RemovalFailed(L10n.RemoveAccountFailed(removal.Detail)),
        _ => ChangeFailed(L10n.ErrorUnknown()),
    };

    /// <summary>A removal whose stored sign-in could not be erased, so it comes back at the next
    /// launch: said rather than left to surprise.</summary>
    private static AccountNotice RemovalFailed(string message) => new(InfoBarSeverity.Error, string.Empty, message);

    /// <summary>The model says a removal's erase failed; said on the list while Accounts is open.</summary>
    private void OnRemovalFailed(string message)
    {
        if (CurrentCategory() == "accounts")
        {
            NotifyAccounts(RemovalFailed(message));
        }
    }

    /// <summary>
    /// The core says the accounts may have changed. The open list or page is drawn again when what
    /// it shows did; the last account gone closes Settings, because the window returns to first-run
    /// setup underneath it.
    /// </summary>
    private void OnAccountsChanged()
    {
        // Every other category reconciles itself, and the core is asked nothing on their behalf:
        // this runs on every settings and connectivity signal. An account is removed from the
        // Accounts category, so that is where the last one goes. Videoconferencing lists the
        // accounts that make meetings, from the same snapshot.
        if (!_rebuilding && CurrentCategory() == "videoconferencing")
        {
            RefreshMeetingAccounts();
            return;
        }
        if (_rebuilding || CurrentCategory() != "accounts")
        {
            return;
        }
        var snapshot = _model.AccountsSnapshot();
        AskSharedAgain(snapshot);
        var any = snapshot.Accounts.Length > 0;
        if (!any && _hadAccounts)
        {
            Hide();
            return;
        }
        _hadAccounts = any;
        var page = OpenPage();
        if (_model.TakeChangeNotice(page) is { } notice)
        {
            _accountNotice = ChangeFailed(notice);
        }
        if (AccountsFingerprint(snapshot, page) != _accountsDrawn)
        {
            ShowCategory("accounts");
        }
        else if (_accountNoticeBar is { } bar)
        {
            ShowNotice(bar, _accountNotice);
        }
    }

    /// <summary>What the list or a page is drawn from: the snapshot, the expired sign-ins, which
    /// accounts have a mailbox listed and how each is shared, the notices about accounts, and the
    /// open page's record, which the page draws from calls of their own.</summary>
    private string AccountsFingerprint(AccountsSnapshot snapshot, AccountPage? page)
    {
        var beside = (_model.GetSyncSettings()?.Accounts.Select(account => "mail:" + account.AccountId) ?? [])
            .Concat(_model.AccountsSyncMode.Select(pair => $"shared:{pair.Key}={pair.Value}"))
            .Concat(AccountNoticesFingerprint())
            .Concat(AccountSettingsRules.PageFingerprint(page));
        return AccountSettingsRules.Fingerprint(snapshot, _model.SignInExpiredIds, beside);
    }

    private string? CurrentCategory() => (_categories.SelectedItem as ListViewItem)?.Tag as string;

    private static Style AccentButton() => (Style)Application.Current.Resources["AccentButtonStyle"];
}
