// What a new-mail pass becomes on screen: who it is from, what it is about, how it begins, and
// then a summary for whatever the core's preview cap left out (docs/background-sync.md). The
// Windows twin of the Linux client's notification_parts and Android's MailNotifier grouping.
//
// WinUI-free and L10n-free on purpose, the words arrive as parameters and the toast itself is
// NewMailNotifier: keying by MESSAGE rather than by account is a contract requirement, and the
// way it fails is silent. A per-account tag would be REPLACED by the next pass's notification,
// so an earlier unseen message would vanish from the shell's notification list with nothing on
// screen looking wrong, and the marks have already advanced past it by then.

using System.Collections.Generic;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>One toast: what it says, and the identity Windows replaces it by.</summary>
/// <param name="Title">The first line, the sender.</param>
/// <param name="Body">The second line, the subject.</param>
/// <param name="Preview">
/// The third line, how the message begins. Empty when the account has no snippet for it yet, and
/// the notification is then two lines, exactly as it was before there was one.
/// </param>
/// <param name="Tag">The notification's own identity, unique per message.</param>
/// <param name="Group">The account the notification belongs to, so a group can be cleared.</param>
internal readonly record struct NewMailNotice(
    string Title, string Body, string Preview, string Tag, string Group);

/// <summary>Projects a background-sync outcome into the toasts to raise for it.</summary>
internal static class NewMailNotices
{
    /// <summary>
    /// The notices for one pass, in account order, newest message first within an account.
    /// </summary>
    /// <param name="outcome">What the core's scan found.</param>
    /// <param name="unknownSender">Title for a message whose header named no sender at all.</param>
    /// <param name="moreMessages">
    /// The overflow summary's title, already formatted with the count it stands for. Its own
    /// copy rather than the unknown-sender line: a single hidden message must not read as one
    /// more subject-less message.
    /// </param>
    public static IReadOnlyList<NewMailNotice> For(
        BackgroundSyncOutcome outcome,
        string unknownSender,
        Func<int, string> moreMessages)
    {
        var notices = new List<NewMailNotice>();
        foreach (var account in outcome.Accounts)
        {
            notices.AddRange(ForAccount(account, unknownSender, moreMessages));
        }
        return notices;
    }

    private static IEnumerable<NewMailNotice> ForAccount(
        AccountNewMail account,
        string unknownSender,
        Func<int, string> moreMessages)
    {
        foreach (var message in account.Messages)
        {
            yield return new NewMailNotice(
                TitleOf(message, unknownSender),
                message.Subject,
                message.Preview,
                "mailcal-" + message.MessageKey,
                account.AccountId);
        }
        // NewCount is the pass's true total; Messages is capped by the core. Only the difference
        // gets a summary, so a pass that fitted says nothing extra.
        var hidden = (int)account.NewCount - account.Messages.Length;
        if (hidden > 0)
        {
            yield return new NewMailNotice(
                moreMessages(hidden),
                account.AccountLabel,
                // The summary stands for messages it does not name, so it has no one body to
                // quote and says nothing rather than quoting an arbitrary one.
                string.Empty,
                "mailcal-account-" + account.AccountId,
                account.AccountId);
        }
    }

    // The display name where the header carried one, else the bare address, else the fallback
    // line: a notification is never blank, the same rule the avatar's monogram follows.
    private static string TitleOf(NewMailPreview message, string unknownSender)
    {
        if (!string.IsNullOrWhiteSpace(message.SenderName))
        {
            return message.SenderName!;
        }
        return string.IsNullOrWhiteSpace(message.Sender) ? unknownSender : message.Sender;
    }
}
