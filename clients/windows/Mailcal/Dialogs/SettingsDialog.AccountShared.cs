// An account's page, its shared mailboxes (docs/shared-mailboxes.md rules 3, 7 and 8): a switch per
// mailbox the account's sign-in opens besides its own, and on Microsoft a field for the address of
// one. Which rows and lines each answer draws is SharedMailboxRules, which the unit suite drives.
//
// The server is asked when the page opens, and again on a core signal only while the answer may
// still change: before it has listed anything, or once the account's uses have changed. The page keeps a slot for the section and fills it when the answer arrives, so the
// answer redraws nothing else and takes nothing the person was typing. A nested ContentDialog is not
// allowed, so switching a mailbox off is confirmed inline, under its switch, as a use is.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // What each account's server last answered; an account absent has not answered.
    private readonly Dictionary<string, SharedMailboxesView> _sharedAnswers = new();

    // The accounts whose server is being asked now, so a signal does not ask again over it.
    private readonly HashSet<string> _sharedAsking = [];

    // What each account was used for when its server was last asked.
    private readonly Dictionary<string, string> _sharedAskedUses = new();

    // The slot the page drawn last keeps for the section, and whose page it is.
    private (string Account, StackPanel Slot)? _sharedDrawn;

    /// <summary>The section's slot, filled from the answer the page has, or once it arrives.</summary>
    private StackPanel SharedMailboxesSlot(AccountEntry entry)
    {
        var slot = new StackPanel();
        _sharedDrawn = (entry.Id, slot);
        _sharedAnswers.TryGetValue(entry.Id, out var view);
        FillShared(slot, entry.Id, view);
        if (view is null && !_sharedAsking.Contains(entry.Id))
        {
            _ = AskSharedAsync(entry);
        }
        return slot;
    }

    /// <summary>Asks the open page's server again when its answer may have changed: the account
    /// has connected since, or what it is used for changed.</summary>
    private void AskSharedAgain(AccountsSnapshot snapshot)
    {
        if (snapshot.Accounts.FirstOrDefault(account => account.Id == _accountPage) is not { } entry)
        {
            return;
        }
        _sharedAnswers.TryGetValue(entry.Id, out var answer);
        var usesChanged = _sharedAskedUses.TryGetValue(entry.Id, out var asked) && asked != SharedMailboxRules.Uses(entry);
        if (SharedMailboxRules.MayHaveChanged(answer, _sharedAsking.Contains(entry.Id), usesChanged))
        {
            _ = AskSharedAsync(entry);
        }
    }

    private Task AskSharedAsync(AccountEntry entry)
    {
        _sharedAskedUses[entry.Id] = SharedMailboxRules.Uses(entry);
        return AskSharedAsync(entry.Id);
    }

    private async Task AskSharedAsync(string accountId)
    {
        _sharedAsking.Add(accountId);
        var view = await _model.SharedMailboxesAsync(accountId);
        _sharedAsking.Remove(accountId);
        var had = _sharedAnswers.TryGetValue(accountId, out var previous);
        if (view is null)
        {
            _sharedAnswers.Remove(accountId);
        }
        else
        {
            _sharedAnswers[accountId] = view;
        }
        // Asked again on every signal while it may change, so only a different answer redraws.
        if ((had || view is not null) && !SharedMailboxRules.Same(previous, view)
            && _sharedDrawn is { } drawn && drawn.Account == accountId)
        {
            FillShared(drawn.Slot, accountId, view);
        }
    }

    private void FillShared(StackPanel slot, string accountId, SharedMailboxesView? view)
    {
        slot.Children.Clear();
        // Collapsed rather than empty: the page's spacing would still count an empty slot.
        slot.Visibility = SharedMailboxRules.Shown(view) ? Visibility.Visible : Visibility.Collapsed;
        if (view is null || !SharedMailboxRules.Shown(view))
        {
            return;
        }
        var stack = new StackPanel { Spacing = 8 };
        foreach (var mailbox in view.Mailboxes)
        {
            stack.Children.Add(SharedRow(accountId, mailbox));
        }
        if (SharedMailboxRules.TakesAddress(view))
        {
            stack.Children.Add(SharedAddress(accountId));
        }
        if (SharedNoteText(SharedMailboxRules.Note(view)) is { } note)
        {
            var line = Description(note);
            AutomationProperties.SetAutomationId(line, "SharedMailboxesNote");
            stack.Children.Add(line);
        }
        var group = Group(L10n.SettingsAccountSharedHeading(), L10n.SettingsAccountSharedDescription(), stack);
        AutomationProperties.SetAutomationId(group, "SharedMailboxes");
        slot.Children.Add(group);
    }

    private static string? SharedNoteText(SharedNote note) => note switch
    {
        SharedNote.NoneShared => L10n.SettingsAccountSharedNone(),
        SharedNote.ByAddress => L10n.SettingsAccountSharedByAddress(),
        SharedNote.NotConnected => L10n.SettingsAccountSharedNotConnected(),
        SharedNote.Unreachable => L10n.SettingsAccountSharedUnreachable(),
        _ => null,
    };

    /// <summary>One mailbox's switch, named by its address, with the server's label or the line
    /// saying it is no longer shared beneath it.</summary>
    private StackPanel SharedRow(string accountId, SharedMailboxEntry mailbox)
    {
        var shown = SharedMailboxRules.Row(mailbox);
        var row = new StackPanel { Spacing = 2 };
        var toggle = new ToggleSwitch { Header = shown.Title, IsOn = shown.Active };
        AutomationProperties.SetAutomationId(toggle, $"SharedMailbox_{shown.Title}");
        row.Children.Add(toggle);
        if ((shown.Withdrawn ? L10n.SettingsAccountSharedWithdrawn() : shown.Label) is { } line)
        {
            row.Children.Add(Description(line));
        }

        // Switching off deletes what the device holds of the mailbox, so it is asked first;
        // cancelling puts the switch back.
        var confirm = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        confirm.Children.Add(Heading(L10n.SettingsAccountSharedOffTitle(shown.Title)));
        confirm.Children.Add(new TextBlock { Text = L10n.SettingsAccountSharedOffMessage(), TextWrapping = TextWrapping.Wrap });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var cancel = new Button { Content = L10n.ActionCancel() };
        var off = new Button { Content = L10n.ActionSwitchOff() };
        AutomationProperties.SetAutomationId(off, "SharedMailboxOff");
        AutomationProperties.SetAutomationId(cancel, "SharedMailboxOffCancel");
        buttons.Children.Add(cancel);
        buttons.Children.Add(off);
        confirm.Children.Add(buttons);
        row.Children.Add(confirm);

        var restoring = false;
        toggle.Toggled += (_, _) =>
        {
            if (_rebuilding || restoring)
            {
                return;
            }
            if (toggle.IsOn)
            {
                toggle.IsEnabled = false;
                _ = SetSharedAsync(accountId, mailbox, true);
            }
            else
            {
                toggle.IsEnabled = false;
                confirm.Visibility = Visibility.Visible;
            }
        };
        cancel.Click += (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            restoring = true;
            toggle.IsOn = true;
            restoring = false;
            toggle.IsEnabled = true;
        };
        off.Click += (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            _ = SetSharedAsync(accountId, mailbox, false);
        };
        return row;
    }

    /// <summary>A field for the address of a mailbox shared with the account, for a server that
    /// lists none. Committed with Enter or Add, never per keystroke: each commit asks the server.</summary>
    private StackPanel SharedAddress(string accountId)
    {
        var field = new TextBox { Header = L10n.SettingsAccountSharedAddress(), MinWidth = 260 };
        field.InputScope = new InputScope { Names = { new InputScopeName(InputScopeNameValue.EmailNameOrAddress) } };
        AutomationProperties.SetAutomationId(field, "SharedMailboxAddress");
        var add = new Button { Content = L10n.ActionAdd(), VerticalAlignment = VerticalAlignment.Bottom };
        AutomationProperties.SetAutomationId(add, "SharedMailboxAdd");
        async Task Add()
        {
            var address = field.Text.Trim();
            if (address.Length == 0 || !add.IsEnabled)
            {
                return;
            }
            field.IsEnabled = add.IsEnabled = false;
            SayOnAccounts(null);
            var found = await _model.ResolveSharedMailboxAsync(accountId, address);
            if (found.Mailbox is { } mailbox)
            {
                await SetSharedAsync(accountId, mailbox, true);
                return;
            }
            // A refusal leaves the address where it was typed, to correct.
            field.IsEnabled = add.IsEnabled = true;
            SayOnAccounts(found.Error is { } error
                ? ChangeFailed(error)
                : new AccountNotice(InfoBarSeverity.Warning, L10n.SettingsAccountSharedNotFound(), string.Empty));
        }
        add.Click += (_, _) => _ = Add();
        field.KeyDown += (_, e) =>
        {
            if (e.Key == Windows.System.VirtualKey.Enter)
            {
                e.Handled = true;
                _ = Add();
            }
        };
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(field);
        line.Children.Add(add);
        return line;
    }

    /// <summary>Uses the account for <paramref name="mailbox"/>, or stops, then draws the page from
    /// what the server answers now.</summary>
    private async Task SetSharedAsync(string accountId, SharedMailboxEntry mailbox, bool on)
    {
        var error = await _model.SetSharedMailboxAsync(accountId, mailbox, on);
        await AskSharedAsync(accountId);
        NotifyAccounts(error is null ? null : ChangeFailed(error));
    }
}
