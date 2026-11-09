// An account's page, what happens to an invitation answer its calendar server cannot pass on: asked
// each time with a notice, emailed, or not (docs/invitations.md). Drawn while the core's page record
// carries it, which is while the account is used for its calendar.

using Allodia.Mailcal.Controls;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private static readonly ReplyFallbackChoice[] ReplyFallbackChoices =
        [ReplyFallbackChoice.Ask, ReplyFallbackChoice.Always, ReplyFallbackChoice.Never];

    /// <summary>The reply setting's picker, through the control components like the link pickers.</summary>
    private UIElement ReplyFallbackGroup(AccountEntry entry, AccountReplyFallback picker)
    {
        var id = entry.Id;
        var box = ControlComponents.ComboBox(
            L10n.SettingsReplyFallbackLabel(),
            [L10n.SettingsReplyFallbackAsk(), L10n.SettingsReplyFallbackAlways(), L10n.SettingsReplyFallbackNever()],
            Math.Max(0, Array.IndexOf(ReplyFallbackChoices, picker.Choice)),
            picker.Control,
            "AccountReplyFallback",
            at => _model.ChangeAccount(new AccountPageIntent.SetReplyFallback(id, ReplyFallbackChoices[at])));
        return Group(L10n.SettingsReplyFallbackHeading(), L10n.SettingsReplyFallbackDescription(), box);
    }
}
