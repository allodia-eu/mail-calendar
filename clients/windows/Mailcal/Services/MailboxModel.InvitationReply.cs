// The "the organiser wasn't told" question, pulled on a Surface.InvitationReply change: a calendar
// server that promised to tell the organiser stored the answer and then reported it could not
// (RFC 6638 §3.2.9, docs/invitations.md). Its own partial to keep MailboxModel.cs under the
// 500-line limit.
//
// Nothing here decides anything. The core raises the question, clears it the instant it is
// answered, and signals both times, so this mirrors what it holds and never dismisses on its own.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private ReplyPrompt? _replyPrompt;

    /// <summary>
    /// The unanswered question, or <c>null</c> when there is nothing to ask, which is also how the
    /// core says <i>close it</i>.
    /// </summary>
    /// <remarks>
    /// <c>internal</c>, like <see cref="CalendarWrite"/> and for the same reason: every UniFFI
    /// record is generated <c>internal</c>, so a <c>public</c> property of one does not compile
    /// (CS0053). Only the code-behind reads it, and that is in this assembly; the two members the
    /// XAML binds, <see cref="HasReplyPrompt"/> and <see cref="ReplyPromptText"/>, are a bool and
    /// a string, and stay public.
    /// </remarks>
    internal ReplyPrompt? ReplyPrompt
    {
        get => _replyPrompt;
        private set
        {
            if (Set(ref _replyPrompt, value))
            {
                Raise(nameof(HasReplyPrompt));
                Raise(nameof(ReplyPromptText));
            }
        }
    }

    /// <summary>Whether the reply-undelivered prompt should show.</summary>
    public bool HasReplyPrompt => ReplyPrompt is not null;

    /// <summary>
    /// The body of the prompt: the meeting that was answered, and the address the email would go
    /// to.
    /// </summary>
    /// <remarks>
    /// The organiser is <b>named</b> rather than called "the organiser", because pressing Send
    /// posts mail from the user's account to somebody they did not choose in this moment, and that
    /// consent is not informed unless the recipient is on screen. The RFC 6638 status code the
    /// prompt also carries is deliberately left out, it is for the log, and "5.2" explains nothing
    /// to the person reading this.
    /// <para>
    /// Both interpolated values are attacker-controlled (a meeting title and an address, from mail
    /// somebody else wrote). This is bound to an InfoBar's <c>Message</c>, which renders text and
    /// never markup, <c>docs/rendering-security.md</c>.
    /// </para>
    /// </remarks>
    public string ReplyPromptText
    {
        get
        {
            var prompt = ReplyPrompt;
            return prompt is null
                ? string.Empty
                : L10n.InvitationReplyUndeliveredBody(prompt.Summary, prompt.Organizer);
        }
    }

    /// <summary>Pulls the pending question from the core (on a <c>Surface.InvitationReply</c>
    /// signal).</summary>
    private void PullReplyPrompt()
    {
        var prompt = _app?.ReplyPrompt();
        ReplyPrompt = prompt;
        // The status code and whether there is a question, never the meeting or the address,
        // which are message content (docs/invitations.md → Logging and privacy).
        Log.Info(prompt is null
            ? "invitation: the reply question is answered"
            : $"invitation: the calendar server could not deliver the reply (status {prompt.StatusCode}); asking");
    }

    /// <summary>
    /// Answers the question: whether to email the reply ourselves, and whether that becomes this
    /// account's standing answer.
    /// </summary>
    /// <remarks>
    /// <paramref name="remember"/> applies to whichever way <paramref name="send"/> went, ticked
    /// beside "Don't send" is a standing <i>no</i>, which is what stops a server that fails every
    /// reply asking again at every meeting.
    /// <para>
    /// Carries no handle on the meeting: the core holds the question and clears it as this
    /// arrives, so pressing twice cannot email the organiser twice. <paramref name="replySubject"/>
    /// comes from the shell for the same reason the RSVP's does, the core carries no locale, and
    /// it is copy a stranger reads in their inbox.
    /// </para>
    /// </remarks>
    internal void AnswerReplyPrompt(bool send, bool remember, string? replySubject)
    {
        // The answer and the two flags only. The subject is not logged: it *contains* the meeting's
        // title, which is message content (docs/invitations.md → Logging and privacy).
        Log.Info($"invitation: reply question answered send={send} remember={remember}");
        _app?.Dispatch(new Intent.AnswerReplyPrompt(send, remember, replySubject));
    }
}
