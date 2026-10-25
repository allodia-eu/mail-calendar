// An account's page, a calendar or address book paused for a server found on another site
// (docs/account-autodetect.md rule 14, docs/accounts.md rule 16): "Review" under the use's note
// opens the question that allows the new server. Inline, because Settings is itself a
// ContentDialog. The open question is kept on the dialog, so a redraw while the new server is
// dialled draws it again, busy, and its answer lands on the one drawn last.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The question open on an account's page: whose, under which use's row, the change
    /// it asks about, and where allowing it stands.</summary>
    private sealed record ServerChangeOpen(
        string AccountId, AccountCapability Row, ServerChange Change, bool Busy, string? Error);

    private ServerChangeOpen? _serverChangeOpen;

    // The question as drawn last, which an allow that ends after a redraw updates.
    private (ServerChangePanel Panel, Button Allow, Button NotNow)? _serverChangeDrawn;

    /// <summary>"Review" and, once pressed, the question under <paramref name="capability"/>'s
    /// row, when <paramref name="note"/> says the use is paused for a server change.</summary>
    private StackPanel? ServerChangeAction(AccountEntry entry, AccountCapability capability, UseNote note)
    {
        var kept = _serverChangeOpen is { } held && held.AccountId == entry.Id && held.Row == capability ? held : null;
        if (!AccountSettingsRules.ReviewsServerChange(note))
        {
            // No longer paused, such as allowed meanwhile: a pause that comes back asks afresh.
            if (kept is not null)
            {
                _serverChangeOpen = null;
            }
            return null;
        }
        var panel = new StackPanel { Spacing = 8 };
        if (kept is not { } open)
        {
            var review = new Button { Content = L10n.ServerChangedReview() };
            AutomationProperties.SetAutomationId(review, $"AccountUseServerChangeReview_{capability}");
            review.Click += (_, _) => _ = OpenServerChangeAsync(entry, capability);
            panel.Children.Add(review);
        }
        else
        {
            panel.Children.Add(ServerChangeQuestion(entry, open));
        }
        return panel;
    }

    private async Task OpenServerChangeAsync(AccountEntry entry, AccountCapability capability)
    {
        var change = await _model.PendingServerChangeAsync(entry.Id);
        // None pending: allowed meanwhile, and the next snapshot redraws the row without it.
        _serverChangeOpen = change is null ? null : new ServerChangeOpen(entry.Id, capability, change, false, null);
        NotifyAccounts(null);
    }

    private StackPanel ServerChangeQuestion(AccountEntry entry, ServerChangeOpen open)
    {
        var question = ServerChangeViews.Panel(ServerChangeViews.Copy(open.Change, entry.Address), titled: true);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        // Neither is the accent button: allowing a server is never the easy press.
        var allow = new Button { Content = L10n.ServerChangedAllow() };
        AutomationProperties.SetAutomationId(allow, "ServerChangeAllow");
        var notNow = new Button { Content = L10n.ServerChangedNotNow() };
        AutomationProperties.SetAutomationId(notNow, "ServerChangeNotNow");
        buttons.Children.Add(allow);
        buttons.Children.Add(notNow);
        question.Panel.Children.Add(buttons);
        _serverChangeDrawn = (question, allow, notNow);
        DrawServerChange(open);

        // "Not now" needs no call: the use stays paused.
        notNow.Click += (_, _) =>
        {
            _serverChangeOpen = null;
            _serverChangeDrawn = null;
            NotifyAccounts(null);
        };
        allow.Click += async (_, _) =>
        {
            _serverChangeOpen = open with { Busy = true, Error = null };
            DrawServerChange(_serverChangeOpen);
            var answer = await _model.AcceptServerChangeAsync(open.AccountId, open.Change);
            if (answer is ServerChangeAnswer.Moved)
            {
                // The page redraws from the snapshot, in which the use is no longer paused.
                _serverChangeOpen = null;
                _serverChangeDrawn = null;
                NotifyAccounts(null);
                return;
            }
            if (answer is ServerChangeAnswer.Replaced replaced)
            {
                // A lookup replaced the question while it was shown, and nothing moved: the
                // redraw asks the new one in its place.
                _serverChangeOpen = _serverChangeOpen is { } shown
                    ? shown with { Change = replaced.Change, Busy = false, Error = null }
                    : null;
                _serverChangeDrawn = null;
                NotifyAccounts(null);
                return;
            }
            _serverChangeOpen = _serverChangeOpen is { } still
                ? still with { Busy = false, Error = L10n.ServerChangedFailed() }
                : null;
            if (_serverChangeOpen is { } failed)
            {
                DrawServerChange(failed);
            }
        };
        return question.Panel;
    }

    private void DrawServerChange(ServerChangeOpen open)
    {
        if (_serverChangeDrawn is not { } drawn)
        {
            return;
        }
        drawn.Panel.Busy(open.Busy);
        drawn.Panel.Say(open.Error);
        drawn.Allow.IsEnabled = drawn.NotNow.IsEnabled = !open.Busy;
    }
}
