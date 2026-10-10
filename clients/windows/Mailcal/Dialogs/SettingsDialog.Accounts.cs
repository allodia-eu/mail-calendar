// The Accounts category: every account, mail or not, from the core's one snapshot, each opening a
// page of its own (docs/accounts.md rule 11, docs/settings.md category 9). The page's sections are
// partials of their own: what the account is used for and linked to (AccountUses), its servers and
// its removal (AccountServers), and its mail settings (AccountMail).
//
// State lives in the core. A change on a page forwards to it off the UI thread, and the page is
// drawn again from the next snapshot; a core signal that changed nothing the page shows leaves it,
// and whatever is being typed on it, alone.

using Allodia.Mailcal.Services;
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

    private sealed record AccountNotice(InfoBarSeverity Severity, string Title, string Message);

    private UIElement BuildAccounts()
    {
        var snapshot = _model.AccountsSnapshot();
        _accountsDrawn = AccountsFingerprint(snapshot);
        var panel = new StackPanel { Spacing = 16 };
        _accountNoticeBar = new InfoBar { IsClosable = true };
        ShowNotice(_accountNoticeBar, _accountNotice);
        _accountNoticeBar.Closed += (_, _) => _accountNotice = null;
        var entry = snapshot.Accounts.FirstOrDefault(account => account.Id == _accountPage);
        if (entry is not null)
        {
            panel.Children.Add(BackToAccounts());
            panel.Children.Add(_accountNoticeBar);
            BuildAccountPage(panel, entry);
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
        var lines = new List<string> { $"{KindLabel(entry.Kind)} · {uses}" };
        lines.AddRange(AccountSettingsRules.Linked(entry)
            .Select(link => L10n.SettingsAccountLinkedLine(UseName(link.Use), link.Address)));
        if (AccountSettingsRules.NeedsPermission(entry))
        {
            lines.Add(L10n.SettingsAccountNeedsPermission());
        }
        return string.Join("\n", lines);
    }

    /// <summary>The label for an account's kind, as the setup form names the same choice.</summary>
    private static string KindLabel(AccountKind kind) => kind switch
    {
        AccountKind.Dav => L10n.AccountKindDav(),
        AccountKind.Jmap => L10n.SetupAccountTypeJmap(),
        AccountKind.Microsoft => L10n.SetupAccountTypeMicrosoft(),
        AccountKind.Google => L10n.SetupAccountTypeGoogle(),
        _ => L10n.SetupAccountTypePassword(),
    };

    /// <summary>A use's name, as the main window's sections name it.</summary>
    internal static string UseName(AccountCapability capability) => capability switch
    {
        AccountCapability.Calendar => L10n.NavCalendar(),
        AccountCapability.Contacts => L10n.NavContacts(),
        AccountCapability.Colleagues => L10n.AccountUseColleagues(),
        _ => L10n.NavMail(),
    };

    /// <summary>One account's page: how it travels, what it is used for, the accounts it relies
    /// on, signing in again or its own servers, its mail settings while it is used for mail, and
    /// its removal, last.</summary>
    private void BuildAccountPage(StackPanel panel, AccountEntry entry)
    {
        var title = new TextBlock
        {
            Text = entry.Address,
            Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        panel.Children.Add(title);
        panel.Children.Add(Description(KindLabel(entry.Kind)));
        // Whether this one travels: first, because it decides whether anything below it is
        // anybody else's business (docs/settings.md).
        if (_model.AccountsSyncMode.TryGetValue(entry.Id, out var mode))
        {
            panel.Children.Add(BuildSyncModePicker(entry.Id, mode));
        }
        panel.Children.Add(UsesGroup(entry));
        if (AccountSettingsRules.LinkPickers(entry) is { Count: > 0 } pickers)
        {
            panel.Children.Add(LinksGroup(entry, pickers));
        }
        var expired = _model.SignInExpiredIds.Contains(entry.Id);
        if (AccountSettingsRules.SignsInAtProvider(entry.Kind))
        {
            panel.Children.Add(SignInGroup(entry, expired));
        }
        if (entry.Endpoints is { } endpoints)
        {
            panel.Children.Add(ServersGroup(entry, endpoints, expired));
        }
        if (MailGroup(entry, expired) is { } mail)
        {
            panel.Children.Add(mail);
        }
        panel.Children.Add(RemoveGroup(entry));
    }

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
    private void SayOnAccounts(AccountNotice notice)
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

    /// <summary>
    /// The core says the accounts may have changed. The open list or page is drawn again when what
    /// it shows did; the last account gone closes Settings, because the window returns to first-run
    /// setup underneath it.
    /// </summary>
    private void OnAccountsChanged()
    {
        // Every other category reconciles itself, and the core is asked nothing on their behalf:
        // this runs on every settings and connectivity signal. An account is removed from the
        // Accounts category, so that is where the last one goes.
        if (_rebuilding || CurrentCategory() != "accounts")
        {
            return;
        }
        var snapshot = _model.AccountsSnapshot();
        var any = snapshot.Accounts.Length > 0;
        if (!any && _hadAccounts)
        {
            Hide();
            return;
        }
        _hadAccounts = any;
        if (AccountsFingerprint(snapshot) != _accountsDrawn)
        {
            ShowCategory("accounts");
        }
    }

    /// <summary>What the list or a page is drawn from: the snapshot, the expired sign-ins, and
    /// which accounts have a mailbox listed and how each is shared, which the page draws from
    /// calls of their own.</summary>
    private string AccountsFingerprint(AccountsSnapshot snapshot)
    {
        var beside = (_model.GetSyncSettings()?.Accounts.Select(account => "mail:" + account.AccountId) ?? [])
            .Concat(_model.AccountsSyncMode.Select(pair => $"shared:{pair.Key}={pair.Value}"));
        return AccountSettingsRules.Fingerprint(snapshot, _model.SignInExpiredIds, beside);
    }

    private string? CurrentCategory() => (_categories.SelectedItem as ListViewItem)?.Tag as string;

    private static Style AccentButton() => (Style)Application.Current.Resources["AccentButtonStyle"];
}
