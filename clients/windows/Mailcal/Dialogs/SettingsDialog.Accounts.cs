// The Accounts category: every account, mail or not, from the core's one snapshot, each opening a
// page of its own (docs/accounts.md rule 11, docs/settings.md category 9). The page's sections are
// partials of their own: what the account is used for and linked to (AccountUses), the accounts
// taking a use from it (AccountLinkedFrom), the mailboxes and calendars shared with it
// (AccountSharedStores), how it signs in (AccountSignIn), its servers and its removal (AccountServers), and its mail settings
// (AccountMail). What a change on it came to is worded in AccountChangeNotices.
//
// State lives in the core. A use, a link and a removal are dispatched and the page says they run
// (docs/waiting.md); every other change forwards to the core off the UI thread. The page is drawn
// again from the next snapshot and page record; a core signal that changed nothing the page shows
// leaves it, and whatever is being typed on it, alone.

using Allodia.Mailcal.Controls;
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
    // The open picker, the field holding unsent text or the unsent form that the page's next
    // redraw waits on.
    private object? _redrawWaitsOn;

    // What the list or the page last said, kept across the redraw a change ends with.
    private AccountNotice? _accountNotice;
    // The notice shown that a change on the page left, which the core withdraws when the page's
    // next change starts.
    private AccountNotice? _changeNotice;
    private InfoBar? _accountNoticeBar;

    // What the open list or page was drawn from, so a signal that changed none of it is ignored.
    private string? _accountsDrawn;

    // What a core signal read to find the list or page moved, handed to the redraw it starts: each
    // read is a call into the core on the UI thread, and the redraw would otherwise ask again.
    private AccountsRead? _accountsRead;

    private sealed record AccountsRead(AccountsSnapshot Snapshot, AccountPage? Page, AllodiaDevices? Devices);

    // Whether there was an account when Settings last looked: the last one going is what closes it.
    private bool _hadAccounts;

    // The confirmation open on the page, so the page drawn again from a core signal draws it open
    // too: a reconnecting account signals throughout, and a question that closed by itself would
    // read as answered.
    private string? _confirmOpen;

    private sealed record AccountNotice(InfoBarSeverity Severity, string Title, string Message);

    private UIElement BuildAccounts()
    {
        var read = _accountsRead;
        _accountsRead = null;
        var snapshot = read?.Snapshot ?? _model.AccountsSnapshot();
        var entry = snapshot.Accounts.FirstOrDefault(account => account.Id == _accountPage);
        // A page whose account has gone is the list.
        var page = entry is null ? null : read is null ? OpenPage() : read.Page;
        var devices = read is null ? _model.AllodiaDevices() : read.Devices;
        _accountsDrawn = AccountsFingerprint(snapshot, page, devices);
        var panel = new StackPanel { Spacing = 16 };
        _accountNoticeBar = new InfoBar { IsClosable = true };
        ShowNotice(_accountNoticeBar, _accountNotice);
        _accountNoticeBar.Closed += (_, _) => _accountNotice = null;
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
        if (BuildAllodiaSync(devices) is { } sync)
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
    /// on and those relying on it, the mailboxes shared with it, how it signs in, its own servers, its mail settings while
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
        if (page.Sharing is { } sharing)
        {
            panel.Children.Add(SharingGroup(entry.Id, sharing));
        }
        if (page.SenderName is { } name)
        {
            panel.Children.Add(SenderNameGroup(entry, name));
        }
        panel.Children.Add(UsesGroup(entry, page));
        if (page.Links.Length > 0)
        {
            panel.Children.Add(LinksGroup(entry, page.Links));
        }
        if (page.LinkedFrom.Length > 0)
        {
            panel.Children.Add(LinkedFromGroup(entry, page.LinkedFrom));
        }
        if (page.ReplyFallback is { } replies)
        {
            panel.Children.Add(ReplyFallbackGroup(entry, replies));
        }
        if (page.Shared is { } shared)
        {
            panel.Children.Add(SharedGroup(entry, shared));
        }
        if (page.SignIn is { } signIn)
        {
            panel.Children.Add(AccountSignInGroup(entry, signIn));
        }
        if (entry.MailSync is { } technology)
        {
            panel.Children.Add(SyncTechnologyGroup(technology));
        }
        if (page.Servers is { } servers)
        {
            panel.Children.Add(ServersGroup(entry, servers));
        }
        if (page.Mail is { } mail)
        {
            panel.Children.Add(MailGroup(entry, mail));
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
        // A page opened again has the core ask its server again: what it takes, and which mailboxes
        // it shares with the account, can have changed meanwhile.
        if (accountId is not null)
        {
            _model.AccountPageOpened(accountId);
        }
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
        // this runs on every settings and notices signal. An account is removed from the
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
        var any = snapshot.Accounts.Length > 0;
        if (!any && _hadAccounts)
        {
            Hide();
            return;
        }
        _hadAccounts = any;
        var page = OpenPage();
        AskUnreached(page);
        if (_model.TakeChangeNotice(page) is { } notice)
        {
            _accountNotice = _changeNotice = ChangeFailed(notice);
        }
        else if (_changeNotice is not null && ReferenceEquals(_accountNotice, _changeNotice) && page?.Notice is null)
        {
            // A change since says nothing of the one that failed: a save that worked must not
            // stand under "Couldn't save".
            _accountNotice = _changeNotice = null;
        }
        var devices = _model.AllodiaDevices();
        if (AccountsFingerprint(snapshot, page, devices) != _accountsDrawn)
        {
            // Drawn again now, a picker whose list is open would close under the pointer and the
            // pick would be lost, and a field holding what is still being typed would send it half
            // finished as it goes. The page is drawn once the list closes or the field is left,
            // which sends what it holds; either change is queued in the core behind this one.
            if (OpenPicker() is { } open)
            {
                WhenClosed(open);
                return;
            }
            if (UnsentField() is { } field)
            {
                WhenLeft(field);
                return;
            }
            if (UnsentForm() is { } form)
            {
                WhenSent(form);
                return;
            }
            _accountsRead = new AccountsRead(snapshot, page, devices);
            ShowCategory("accounts");
        }
        else if (_accountNoticeBar is { } bar)
        {
            ShowNotice(bar, _accountNotice);
        }
    }

    /// <summary>The picker on the page whose list is open, if any.</summary>
    private ComboBox? OpenPicker()
    {
        var pending = new Stack<DependencyObject>([_detail]);
        while (pending.TryPop(out var node))
        {
            if (node is ComboBox { IsDropDownOpen: true } open)
            {
                return open;
            }
            for (var i = 0; i < Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(node); i++)
            {
                pending.Push(Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(node, i));
            }
        }
        return null;
    }

    /// <summary>The text field with the keyboard, if it holds text it has not sent.</summary>
    private TextBox? UnsentField() =>
        XamlRoot is { } root
        && Microsoft.UI.Xaml.Input.FocusManager.GetFocusedElement(root) is TextBox box
        && ControlComponents.HoldsUnsentText(box)
            ? box
            : null;

    /// <summary>A form on the page holding edits its button has not sent, if any.</summary>
    private ControlComponents.FormState? UnsentForm()
    {
        var pending = new Stack<DependencyObject>([_detail]);
        while (pending.TryPop(out var node))
        {
            if (node is FrameworkElement { Tag: ControlComponents.FormState { Unsent: true } form })
            {
                return form;
            }
            for (var i = 0; i < Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(node); i++)
            {
                pending.Push(Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(node, i));
            }
        }
        return null;
    }

    /// <summary>Asks for the page again once <paramref name="form"/> is sent, once however many
    /// signals arrive meanwhile.</summary>
    private void WhenSent(ControlComponents.FormState form)
    {
        if (ReferenceEquals(_redrawWaitsOn, form))
        {
            return;
        }
        _redrawWaitsOn = form;
        void Sent()
        {
            form.Sent -= Sent;
            _redrawWaitsOn = null;
            OnAccountsChanged();
        }
        form.Sent += Sent;
    }

    /// <summary>Asks for the page again once <paramref name="picker"/>'s list closes, once
    /// however many signals arrive while it is open.</summary>
    private void WhenClosed(ComboBox picker)
    {
        if (ReferenceEquals(_redrawWaitsOn, picker))
        {
            return;
        }
        _redrawWaitsOn = picker;
        void Closed(object? sender, object args)
        {
            picker.DropDownClosed -= Closed;
            _redrawWaitsOn = null;
            OnAccountsChanged();
        }
        picker.DropDownClosed += Closed;
    }

    /// <summary>Asks for the page again once <paramref name="field"/> loses the keyboard, after it
    /// has sent what it holds, once however many signals arrive meanwhile.</summary>
    private void WhenLeft(TextBox field)
    {
        if (ReferenceEquals(_redrawWaitsOn, field))
        {
            return;
        }
        _redrawWaitsOn = field;
        void Left(object sender, RoutedEventArgs args)
        {
            field.LostFocus -= Left;
            _redrawWaitsOn = null;
            OnAccountsChanged();
        }
        field.LostFocus += Left;
    }

    /// <summary>What the list or a page is drawn from: the snapshot, what the other devices said
    /// while the list is open, the notices about accounts, and the open page's record, which the
    /// page draws from calls of their own.</summary>
    private string AccountsFingerprint(AccountsSnapshot snapshot, AccountPage? page, AllodiaDevices? devices)
    {
        var beside = AccountSettingsRules.CardFingerprint(page, devices)
            .Concat(AccountNoticesFingerprint())
            .Concat(ServerChangeFingerprint())
            .Concat(AccountSettingsRules.PageFingerprint(page));
        return AccountSettingsRules.Fingerprint(snapshot, beside);
    }

    private string? CurrentCategory() => (_categories.SelectedItem as ListViewItem)?.Tag as string;

    private static Style AccentButton() => (Style)Application.Current.Resources["AccentButtonStyle"];
}
