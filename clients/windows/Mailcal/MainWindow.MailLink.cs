// Opening a mail link (`mailto:`) in the composer, the shell half of what the OS hands us.
//
// The gate that decides an activation IS a mail link is MailLink (Services/, pure and unit-tested);
// the decode is the shared core's parse_mailto_uri, so the header allowlist and the injection
// defences are one implementation on every platform (docs/composer-security.md, Gate 12). What is
// left here is what only the shell knows: whether there is an account to send from yet, whether a
// draft is already open, and which surface is in front of the composer.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // The decoded link waiting for a composer. Non-null only between arriving and opening, which
    // is not always instant: a link that arrives before any account exists is held until one does,
    // so a mail link tapped on a fresh install opens once setup finishes rather than vanishing.
    private MailtoPrefill? _pendingMailLink;

    // Guards the await in TryOpenPendingMailLink. A second link arriving while the discard prompt
    // is up must not open a second composer behind the question the user is still answering.
    private bool _openingMailLink;

    /// <summary>
    /// Opens the composer on a mail link, pre-filled with what the link named and editable.
    /// </summary>
    /// <remarks>
    /// Nothing here is sent: a link fills a composer in, the user still writes and presses Send.
    /// A URI the core does not recognise as a mail link is dropped, the launch is not ours to act
    /// on, and an error the user cannot do anything about is worse than nothing.
    /// </remarks>
    internal void OpenMailLink(string uri)
    {
        // The URI is message content end to end (recipients, subject, body), so this records THAT
        // one arrived and nothing of what it said (docs/logging.md).
        if (MailcalBindingsMethods.ParseMailtoUri(uri) is not { } prefill)
        {
            Log.Info("mail link ignored: not a mail link");
            return;
        }
        Log.Info("mail link received");
        _pendingMailLink = prefill;
        TryOpenPendingMailLink();
    }

    // Opens the held link if the app is in a state to compose. Called on arrival, once at startup
    // for a cold start, and whenever the account list changes, the last of which is what lets a
    // link that arrived before the first account still open, without polling for one.
    private async void TryOpenPendingMailLink()
    {
        if (_pendingMailLink is not { } prefill || _openingMailLink)
        {
            return;
        }
        // No account, nothing to send from, and a composer whose From is blank is the failure the
        // picker exists to prevent. Keep the link; adding an account brings us back here.
        if (Model.Accounts.Count == 0)
        {
            return;
        }
        _openingMailLink = true;
        try
        {
            // A link arrives unprompted, at any moment, the same footing as an assistant's draft
            // (docs/mcp.md), and behind the same guard. Someone else's suggestion may not throw
            // away a message the user is in the middle of writing.
            if (!await ConfirmDiscardDraftAsync())
            {
                Log.Info("mail link declined, the open draft was kept");
                _pendingMailLink = null;
                return;
            }
            _pendingMailLink = null;
            // The composer lives in the mail surface's detail column, so a link arriving over the
            // calendar or Contacts would otherwise open it behind them and the click would look
            // like it did nothing.
            Model.ShowMail();
            BeginCompose(new ComposeRequest(
                RichComposeKind.New,
                Account: null,
                Key: null,
                // A link never names a sender, `from` is the header Gate 12 drops first, so the
                // app-level default decides, exactly as a user-initiated new message does.
                InitialFrom: Model.SendAccount(Model.SelectedAccount)?.Id,
                InitialTo: prefill.To,
                InitialCc: prefill.Cc,
                Quote: null,
                QuoteStyle: Model.QuoteSettings.Style,
                QuoteStylePerMessage: Model.QuoteSettings.PerMessage,
                InitialBcc: prefill.Bcc,
                InitialSubject: prefill.Subject,
                // Empty means "no body seed", a bare link opens the composer as blank as the New
                // button does, rather than seeding an empty paragraph over the signature.
                InitialBody: string.IsNullOrEmpty(prefill.Body) ? null : prefill.Body));
            // The request came from a browser or another app, so this process does not hold
            // foreground rights and a bare Activate() would be ignored.
            BringToForeground();
        }
        finally
        {
            _openingMailLink = false;
        }
    }
}
