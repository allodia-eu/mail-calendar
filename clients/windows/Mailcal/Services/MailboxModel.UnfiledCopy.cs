// The "your message went out but its copy is not in Sent" question, pulled on a
// Surface.UnfiledCopy change. Its own partial to keep MailboxModel.cs under the 500-line limit.
//
// Nothing here decides anything. The core raises the question, clears it the instant the copy is
// filed or the user dismisses it, and signals both times, so this mirrors what it holds and never
// closes on its own.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private UnfiledCopy? _unfiledCopy;

    /// <summary>
    /// The message whose Sent copy is missing, or <c>null</c> when there is nothing to ask, which
    /// is also how the core says <i>close it</i>.
    /// </summary>
    /// <remarks>
    /// <c>internal</c> for the same reason as <see cref="ReplyPrompt"/>: every UniFFI record is
    /// generated <c>internal</c>, so a <c>public</c> property of one does not compile (CS0053).
    /// The members the XAML binds are a bool and a string, and stay public.
    /// </remarks>
    internal UnfiledCopy? UnfiledCopy
    {
        get => _unfiledCopy;
        private set
        {
            if (Set(ref _unfiledCopy, value))
            {
                Raise(nameof(HasUnfiledCopy));
                Raise(nameof(UnfiledCopyText));
                Raise(nameof(UnfiledCopyBusy));
                // The one the buttons actually bind their IsEnabled to. Left unraised, both
                // keep the value they were evaluated with when the question did not exist yet
                // They would remain permanently disabled, with nothing the user can press.
                Raise(nameof(UnfiledCopyActionable));
            }
        }
    }

    /// <summary>Whether the missing-Sent-copy question should show.</summary>
    public bool HasUnfiledCopy => UnfiledCopy is not null;

    /// <summary>Whether a filing attempt is in flight, so both buttons disable rather than let the
    /// user queue five of them.</summary>
    public bool UnfiledCopyBusy => UnfiledCopy?.Retrying == true;

    /// <summary>Whether the buttons are pressable, the inverse of <see cref="UnfiledCopyBusy"/>,
    /// because XAML has no negation in a binding.</summary>
    public bool UnfiledCopyActionable => HasUnfiledCopy && !UnfiledCopyBusy;

    /// <summary>
    /// The body: which message reached its recipients without leaving a copy behind.
    /// </summary>
    /// <remarks>
    /// It says the message <b>was sent</b>, because it was. Wording this as a failed send would
    /// make the reader's next move "send it again", the one action that would actually hurt. The
    /// provider detail the question also carries is deliberately left out: it is for the log, and
    /// "connection reset by peer" explains nothing to the person reading this.
    /// </remarks>
    public string UnfiledCopyText
    {
        get
        {
            var unfiled = UnfiledCopy;
            return unfiled is null ? string.Empty : L10n.UnfiledCopyBody(unfiled.Subject);
        }
    }

    /// <summary>Pulls the pending question from the core (on a <c>Surface.UnfiledCopy</c>
    /// signal).</summary>
    private void PullUnfiledCopy()
    {
        var unfiled = _app?.UnfiledCopy();
        UnfiledCopy = unfiled;
        // Whether there is a question and why filing failed, never the subject, which is the
        // user's own mail (docs/logging.md).
        Log.Info(unfiled is null
            ? "sent copy: the missing-copy question is answered"
            : $"sent copy: a delivered message was not filed in Sent ({unfiled.Detail}); asking");
    }

    /// <summary>
    /// Files the Sent copy of the already-delivered message. <b>Sends nothing</b>, the message
    /// left when it was submitted; this places the copy the send could not.
    /// </summary>
    /// <remarks>
    /// Carries no handle on the message: the core holds the one it is asking about, so pressing
    /// twice cannot file two copies.
    /// </remarks>
    public void RetryUnfiledCopy() => _app?.Dispatch(new Intent.RetryUnfiledCopy());

    /// <summary>Accepts the missing copy and closes the question. The message stays sent.</summary>
    public void DismissUnfiledCopy() => _app?.Dispatch(new Intent.DismissUnfiledCopy());
}
