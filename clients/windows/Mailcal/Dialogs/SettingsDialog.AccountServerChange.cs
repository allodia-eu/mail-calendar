// An account's page, a calendar or address book paused for a server found on another site
// (docs/account-autodetect.md rule 14, docs/accounts.md rule 16): "Review" under the use's note
// opens the question that allows the new server. Inline, because Settings is itself a
// ContentDialog. Which question is open is the dialog's; what it says, whether Allow runs and
// whether the last one failed are the core's record, so a redraw draws it as it stands.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The question open on an account's page: whose, and under which use's row.</summary>
    private (string AccountId, AccountCapability Row)? _serverChangeOpen;

    /// <summary>"Review" and, once pressed, the question under <paramref name="capability"/>'s
    /// row, when the core offers it there (<paramref name="reviews"/>): under the last of the uses
    /// paused for a server change.</summary>
    private StackPanel? ServerChangeAction(
        AccountEntry entry, AccountCapability capability, bool reviews, ControlState control)
    {
        var open = _serverChangeOpen is { } held && held.AccountId == entry.Id && held.Row == capability;
        var question = open ? _model.ServerChangeQuestion(entry.Id) : null;
        if (!reviews || (open && question is null))
        {
            // No longer paused, such as allowed meanwhile: a pause that comes back asks afresh.
            if (open)
            {
                _serverChangeOpen = null;
            }
            return null;
        }
        var panel = new StackPanel { Spacing = 8 };
        if (question is null)
        {
            panel.Children.Add(ControlComponents.Button(
                L10n.ServerChangedReview(), control, $"AccountUseServerChangeReview_{capability}",
                () =>
                {
                    _serverChangeOpen = (entry.Id, capability);
                    NotifyAccounts(null);
                }));
        }
        else
        {
            var (drawn, notNow) = ServerChangeViews.Question(
                question, titled: true,
                () => _model.AllowServerChange(entry.Id, question.Change),
                // "Not now" needs no call: the use stays paused.
                () =>
                {
                    _serverChangeOpen = null;
                    NotifyAccounts(null);
                });
            // A redraw replaces the Allow just pressed, so the focus goes to "Not now" rather
            // than wherever the page puts it.
            drawn.Loaded += (_, _) => notNow.Focus(FocusState.Programmatic);
            panel.Children.Add(drawn);
        }
        return panel;
    }

    /// <summary>The open question as the core draws it, for the page's fingerprint: a lookup that
    /// replaces it moves nothing else on the page.</summary>
    private IEnumerable<string> ServerChangeFingerprint()
    {
        if (_serverChangeOpen is { } open && _model.ServerChangeQuestion(open.AccountId) is { } question)
        {
            yield return $"server-change:{open.AccountId}/{open.Row}/{question}";
        }
    }
}
