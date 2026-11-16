// An account's page, its shared mailboxes and calendars (docs/shared-mailboxes.md rules 3, 7, 8 and
// 10): a switch per store the account's sign-in opens besides its own, saying what it holds, the
// line under them and, on a server that lists none, a field for one's address. The core asks the
// server and keeps its answer. A nested ContentDialog is not allowed, so switching a store off is
// confirmed inline, under its switch, as a use is.

using Allodia.Mailcal.Controls;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The section, as the page's record has it.</summary>
    private UIElement SharedGroup(AccountEntry entry, AccountSharedStores shared)
    {
        var stack = new StackPanel { Spacing = 8 };
        foreach (var store in shared.Stores)
        {
            stack.Children.Add(SharedRow(entry.Id, store));
        }
        if (shared.Address is { } address)
        {
            stack.Children.Add(SharedAddress(entry.Id, address));
        }
        if (shared.Note is { } note)
        {
            var line = Description(SharedNoteText(note));
            AutomationProperties.SetAutomationId(line, "SharedMailboxesNote");
            stack.Children.Add(line);
        }
        var group = Group(L10n.SettingsAccountSharedHeading(), L10n.SettingsAccountSharedDescription(), stack);
        AutomationProperties.SetAutomationId(group, "SharedMailboxes");
        return group;
    }

    private static string SharedNoteText(SharedStoresNote note) => note switch
    {
        SharedStoresNote.NotConnected => L10n.SettingsAccountSharedNotConnected(),
        SharedStoresNote.NoneShared => L10n.SettingsAccountSharedNone(),
        SharedStoresNote.ByAddress => L10n.SettingsAccountSharedByAddress(),
        _ => L10n.SettingsAccountSharedUnreachable(),
    };

    /// <summary>What a store holds, in words.</summary>
    internal static string SharedHoldsText(SharedStoreHolds holds) => holds switch
    {
        SharedStoreHolds.Mail => L10n.SettingsAccountSharedHoldsMail(),
        SharedStoreHolds.Calendar => L10n.SettingsAccountSharedHoldsCalendar(),
        _ => L10n.SettingsAccountSharedHoldsBoth(),
    };

    /// <summary>What a store's switch says beneath its title: that the server no longer lists it,
    /// or its label beside what it holds.</summary>
    internal static string SharedStoreNote(AccountSharedStore store)
    {
        if (store.Withdrawn)
        {
            return L10n.SettingsAccountSharedWithdrawn();
        }
        var holds = SharedHoldsText(store.Holds);
        return store.Label is { } label ? $"{label} · {holds}" : holds;
    }

    /// <summary>One store's switch, named by its address, with what it holds and the server's
    /// label, or the line saying it is no longer shared, beneath it. Switching off deletes what the
    /// device holds of the store, so it is asked first; cancelling puts the switch back.</summary>
    private StackPanel SharedRow(string accountId, AccountSharedStore store)
    {
        void Set(bool on) => _model.ChangeAccount(new AccountPageIntent.SetSharedStore(accountId, store.Handle, on));
        var asking = $"shared:{accountId}:{store.Handle}";
        ControlToggle? toggle = null;
        var confirm = new ConfirmPanel(
            L10n.SettingsAccountSharedOffTitle(store.Title),
            L10n.SettingsAccountSharedOffMessage(),
            L10n.ActionSwitchOff(),
            "SharedMailboxOff",
            yes =>
            {
                _confirmOpen = null;
                if (yes)
                {
                    Set(false);
                }
                else
                {
                    toggle?.Restore();
                }
            });
        toggle = new ControlToggle(
            store.Title,
            SharedStoreNote(store),
            store.On,
            store.Control,
            $"SharedMailbox_{store.Title}",
            on =>
            {
                if (_rebuilding)
                {
                    return;
                }
                if (on)
                {
                    Set(true);
                }
                else
                {
                    _confirmOpen = asking;
                    toggle!.Hold();
                    confirm.Show();
                }
            });
        if (_confirmOpen == asking && store.On)
        {
            toggle.HoldOff();
            confirm.Show();
        }
        else if (_confirmOpen == asking)
        {
            _confirmOpen = null;
        }
        confirm.Enabled = store.Control.Enabled;
        var row = new StackPanel { Spacing = 2 };
        row.Children.Add(toggle.Element);
        row.Children.Add(confirm.Element);
        return row;
    }

    /// <summary>A field for the address of a store shared with the account, for a server that
    /// lists none, drawn with the address being added or the one the last add found nothing at,
    /// and Add beside it, as on every other client. Sent with Enter or Add, never per keystroke:
    /// each asks the server. The page is not drawn again while it holds an address Add has not
    /// sent.</summary>
    private StackPanel SharedAddress(string accountId, AccountSharedAddress address)
    {
        var field = new TextBox { Header = L10n.SettingsAccountSharedAddress(), MinWidth = 260, Text = address.Text };
        field.InputScope = new InputScope { Names = { new InputScopeName(InputScopeNameValue.EmailNameOrAddress) } };
        AutomationProperties.SetAutomationId(field, "SharedMailboxAddress");
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(field);
        var state = ControlComponents.Form(line);
        void Send()
        {
            state.Unsent = false;
            _model.ChangeAccount(new AccountPageIntent.AddSharedStore(accountId, field.Text.Trim()));
        }
        var add = ControlComponents.FormButton(L10n.ActionAdd(), address.Add, "SharedMailboxAdd", null, Send);
        void Follow()
        {
            state.Unsent = field.Text != address.Text;
            // Nothing to add yet: Add waits, with nothing to say.
            add.Ready(field.Text.Trim().Length == 0 ? string.Empty : null);
        }
        field.TextChanged += (_, _) => Follow();
        field.KeyDown += (_, e) =>
        {
            if (e.Key == Windows.System.VirtualKey.Enter)
            {
                e.Handled = true;
                if (address.Add.Enabled && field.Text.Trim().Length > 0)
                {
                    Send();
                }
            }
        };
        Follow();
        // Level with the field's box rather than its header.
        add.Element.VerticalAlignment = VerticalAlignment.Bottom;
        line.Children.Add(add.Element);
        return line;
    }
}
