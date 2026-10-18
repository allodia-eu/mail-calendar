// An account's page, a calendar or address book whose server needs a sign-in of its own or refused
// the account's (docs/accounts.md rules 16 and 18): an action under the use's note opens the form
// that moves them to an account of their own, linked to this one. Inline, because Settings is
// itself a ContentDialog. The open form is kept on the dialog, so a redraw while it connects (the
// core adds the new account before it knows the mailbox will take the change) draws it again with
// what was typed and what went wrong.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The form open on an account's page: whose, under which use's row, and what it
    /// holds besides the password, which a redraw does not keep.</summary>
    private sealed record SeparateServerOpen(string AccountId, AccountCapability Row, string Username, string? Error, bool Busy);

    private SeparateServerOpen? _separateOpen;

    // The form as drawn last, which a connect that ends after a redraw updates.
    private SeparateServerForm? _separateDrawn;

    /// <summary>The action and its form under <paramref name="capability"/>'s row, when the core
    /// names a server that use needs a sign-in of its own at; null otherwise.</summary>
    private StackPanel? SeparateServerAction(AccountEntry entry, AccountCapability capability)
    {
        var step = UnreachedUsesRules.Step(entry);
        if (step is null || !UnreachedUsesRules.Offers(step, capability))
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 8 };
        var action = new Button { Content = L10n.SettingsAccountUseSeparateSignIn() };
        AutomationProperties.SetAutomationId(action, $"AccountUseSeparateSignIn_{capability}");
        panel.Children.Add(action);
        var open = _separateOpen is { } kept && kept.AccountId == entry.Id && kept.Row == capability ? kept : null;
        action.Visibility = open is null ? Visibility.Visible : Visibility.Collapsed;
        action.Click += (_, _) =>
        {
            _separateOpen = new SeparateServerOpen(entry.Id, capability, step.Username, null, false);
            NotifyAccounts(null);
        };
        if (open is not null)
        {
            panel.Children.Add(SeparateServerFormFor(step, open));
        }
        return panel;
    }

    private StackPanel SeparateServerFormFor(SeparateServerStep step, SeparateServerOpen open)
    {
        var form = UnreachedUsesViews.Form(step, L10n.ActionCancel(), open.Username);
        _separateDrawn = form;
        form.Say(open.Error);
        // Read from the dialog, not this drawing: a redraw while a connect runs draws it busy, and
        // the connect's end is what frees it.
        void Enable() => form.Connect.IsEnabled = _separateOpen?.Busy != true && form.Password.Password.Length > 0;
        form.Busy(open.Busy);
        Enable();
        form.Password.PasswordChanged += (_, _) => Enable();
        form.Secondary.Click += (_, _) =>
        {
            _separateOpen = null;
            NotifyAccounts(null);
        };
        form.Connect.Click += async (_, _) =>
        {
            var username = form.Username.Text.Trim();
            _separateOpen = open with { Username = username, Error = null, Busy = true };
            form.Say(null);
            form.Busy(true);
            var error = await _model.AddLinkedCalendarAccountAsync(
                step.AccountId, step.Uses, username, form.Password.Password);
            if (error is null)
            {
                // The page redraws from the snapshot, which no longer has the uses on this account
                // and shows the link to the new one.
                _separateOpen = null;
                _separateDrawn = null;
                NotifyAccounts(null);
                return;
            }
            _separateOpen = _separateOpen is { } still ? still with { Error = error, Busy = false } : null;
            if (_separateDrawn is { } drawn)
            {
                drawn.Say(error);
                drawn.Busy(false);
                drawn.Connect.IsEnabled = drawn.Password.Password.Length > 0;
            }
        };
        return form.Panel;
    }
}
