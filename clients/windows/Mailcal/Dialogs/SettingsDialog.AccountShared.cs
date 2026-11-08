// An account's page, its shared mailboxes (docs/shared-mailboxes.md rules 3, 7 and 8), drawn from
// the page's record: a switch per mailbox the account's sign-in opens besides its own, the line
// under them and, on a server that lists none, a field for one's address. The core asks the server
// and keeps its answer. A nested ContentDialog is not allowed, so switching a mailbox off is
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
    private UIElement SharedGroup(AccountEntry entry, AccountSharedMailboxes shared)
    {
        var stack = new StackPanel { Spacing = 8 };
        foreach (var mailbox in shared.Mailboxes)
        {
            stack.Children.Add(SharedRow(entry.Id, mailbox));
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

    private static string SharedNoteText(SharedMailboxesNote note) => note switch
    {
        SharedMailboxesNote.NoneShared => L10n.SettingsAccountSharedNone(),
        SharedMailboxesNote.ByAddress => L10n.SettingsAccountSharedByAddress(),
        SharedMailboxesNote.NotConnected => L10n.SettingsAccountSharedNotConnected(),
        _ => L10n.SettingsAccountSharedUnreachable(),
    };

    /// <summary>One mailbox's switch, named by its address, with the server's label or the line
    /// saying it is no longer shared beneath it. Switching off deletes what the device holds of
    /// the mailbox, so it is asked first; cancelling puts the switch back.</summary>
    private StackPanel SharedRow(string accountId, AccountSharedMailbox mailbox)
    {
        void Set(bool on) => _model.ChangeAccount(new AccountPageIntent.SetSharedMailbox(accountId, mailbox.Handle, on));
        var asking = $"shared:{accountId}:{mailbox.Handle}";
        ControlToggle? toggle = null;
        var confirm = new ConfirmPanel(
            L10n.SettingsAccountSharedOffTitle(mailbox.Title),
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
            mailbox.Title,
            mailbox.Withdrawn ? L10n.SettingsAccountSharedWithdrawn() : mailbox.Label,
            mailbox.On,
            mailbox.Control,
            $"SharedMailbox_{mailbox.Title}",
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
        if (_confirmOpen == asking && mailbox.On)
        {
            toggle.HoldOff();
            confirm.Show();
        }
        else if (_confirmOpen == asking)
        {
            _confirmOpen = null;
        }
        confirm.Enabled = mailbox.Control.Enabled;
        var row = new StackPanel { Spacing = 2 };
        row.Children.Add(toggle.Element);
        row.Children.Add(confirm.Element);
        return row;
    }

    /// <summary>A field for the address of a mailbox shared with the account, for a server that
    /// lists none, drawn with the address being added or the one the last add found nothing at.
    /// Sent with Enter or Add, never per keystroke: each asks the server. The page is not drawn
    /// again while it holds an address Add has not sent.</summary>
    private StackPanel SharedAddress(string accountId, AccountSharedAddress address)
    {
        var field = new TextBox { Header = L10n.SettingsAccountSharedAddress(), MinWidth = 260, Text = address.Text };
        field.InputScope = new InputScope { Names = { new InputScopeName(InputScopeNameValue.EmailNameOrAddress) } };
        AutomationProperties.SetAutomationId(field, "SharedMailboxAddress");
        var line = new StackPanel { Spacing = 8 };
        line.Children.Add(field);
        var state = ControlComponents.Form(line);
        var add = ControlComponents.FormButton(L10n.ActionAdd(), address.Add, "SharedMailboxAdd", null, () =>
        {
            state.Unsent = false;
            _model.ChangeAccount(new AccountPageIntent.AddSharedMailbox(accountId, field.Text.Trim()));
        });
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
                    state.Unsent = false;
                    _model.ChangeAccount(new AccountPageIntent.AddSharedMailbox(accountId, field.Text.Trim()));
                }
            }
        };
        Follow();
        line.Children.Add(add.Element);
        return line;
    }
}
