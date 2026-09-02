// Opening the composer for a share (docs/os-integration.md).
//
// The twin of MainWindow.MailLink.cs, and it behaves identically where the two meet: a share
// arriving before the first account is held until one exists, and one arriving over a draft asks
// before replacing it, because a launch the user did not aim at this window must never throw away
// what they were writing.
//
// What differs is the payload: the composer opens already holding attachments, which no other
// route does. Their names and media types were decided by the shared core, so nothing here
// inspects a file.
using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // The decoded share waiting for a composer. Non-null only between arriving and opening, which
    // is not always instant: a share that arrives before any account exists is held until one
    // does, so sharing a file on a fresh install opens once setup finishes rather than vanishing.
    private SharePrefill? _pendingShare;

    // Guards the await in TryOpenPendingShare, the same way _openingMailLink does: a second share
    // arriving while the discard prompt is up must not open a second composer behind the question
    // the user is still answering.
    private bool _openingShare;

    /// <summary>
    /// Opens the composer on a share, holding what was shared and fully editable.
    /// </summary>
    /// <remarks>
    /// Nothing here is sent: a share fills a composer in, the user still writes and presses Send.
    /// A share carrying nothing usable is dropped rather than answered with a blank composer over
    /// whatever the user was doing, because they asked to send <em>those files</em>.
    /// </remarks>
    internal void OpenShare(SharePrefill prefill)
    {
        if (prefill.IsEmpty)
        {
            Log.Info("share carried nothing to open a composer with");
            return;
        }
        // Counts only: the names are the user's own files (docs/logging.md).
        Log.Info($"share received: {prefill.Attachments.Count} file(s), {prefill.Rejected.Count} refused");
        _pendingShare = prefill;
        TryOpenPendingShare();
    }

    // Opens the held share if the app is in a state to compose. Called on arrival, once at startup
    // for a cold start, and whenever the account list changes, the last of which is what lets a
    // share that arrived before the first account still open, without polling for one.
    private async void TryOpenPendingShare()
    {
        if (_pendingShare is not { } prefill || _openingShare)
        {
            return;
        }
        // No account, nothing to send from, and a composer whose From is blank is the failure the
        // picker exists to prevent. Keep the share; adding an account brings us back here.
        if (Model.Accounts.Count == 0)
        {
            return;
        }
        _openingShare = true;
        try
        {
            var proceed = await ConfirmDiscardDraftAsync();
            ClearPendingShare(prefill);
            if (!proceed)
            {
                Log.Info("share declined, the open draft was kept");
            }
            else
            {
                // The composer lives in the mail surface's detail column, so a share arriving over
                // the calendar or Contacts would otherwise open it behind them.
                Model.ShowMail();
                BeginCompose(new ComposeRequest(
                    RichComposeKind.New,
                    Account: null,
                    Key: null,
                    InitialFrom: Model.SendAccount(Model.SelectedAccount)?.Id,
                    // Non-empty only when the shared text was itself a mail link: a sharing app
                    // cannot otherwise address a message.
                    InitialTo: prefill.To,
                    InitialCc: prefill.Cc,
                    Quote: null,
                    QuoteStyle: Model.QuoteSettings.Style,
                    QuoteStylePerMessage: Model.QuoteSettings.PerMessage,
                    InitialBcc: prefill.Bcc,
                    InitialSubject: prefill.Subject,
                    InitialBody: string.IsNullOrEmpty(prefill.Body) ? null : prefill.Body,
                    Attachments: prefill.Attachments.ToList()));
                // The request came from another app, so this process does not hold foreground
                // rights and a bare Activate() would be ignored.
                BringToForeground();
            }
        }
        finally
        {
            _openingShare = false;
        }
        // A share that arrived while the discard prompt was up is still held: open it now rather
        // than leaving it for the next account change, which on a settled app never comes.
        if (_pendingShare is not null)
        {
            TryOpenPendingShare();
        }
    }

    // Spends the share this pass took, and only that one. A second share arriving during the
    // discard prompt has already replaced `_pendingShare`, and clearing unconditionally would
    // throw away a set of files the user watched leave a share sheet.
    private void ClearPendingShare(SharePrefill taken)
    {
        if (ReferenceEquals(_pendingShare, taken))
        {
            _pendingShare = null;
        }
    }

    /// <summary>Drains a share held from a cold start, once the core and the accounts are up.</summary>
    internal void TakePendingShare()
    {
        if (ShareInbox.Take() is { } prefill)
        {
            OpenShare(prefill);
        }
    }
}
