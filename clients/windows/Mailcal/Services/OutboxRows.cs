// The Outbox list's decisions: what each queued send says about itself, what its menu offers, which
// item asks first and which intent each item sends (docs/sending.md).
//
// WinUI-free and L10n-free on purpose, so Mailcal.Tests can link it. The failure it exists to
// catch is silent in the running app: a state mapped to the wrong word tells someone their
// message is waiting when it is already on its way, or offers "Send now" on a message that may
// have been delivered, which is how it arrives twice. Neither looks wrong on screen.

using System.Collections.Generic;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>What each queued state says and draws, supplied by the shell.</summary>
/// <param name="Waiting">"Waiting to send": the ordinary state, stated plainly.</param>
/// <param name="Sending">"Sending…": a submission is in flight.</param>
/// <param name="Unconfirmed">"Delivery not confirmed": it may already have arrived.</param>
/// <param name="NotSent">"Not sent": the server refused it.</param>
/// <param name="WaitingGlyph">The Segoe Fluent glyph for <paramref name="Waiting"/>.</param>
/// <param name="SendingGlyph">The glyph for <paramref name="Sending"/>.</param>
/// <param name="UnconfirmedGlyph">The glyph for <paramref name="Unconfirmed"/>: a warning, since
/// it is the one state a person may need to check on another device.</param>
/// <param name="NotSentGlyph">The glyph for <paramref name="NotSent"/>: an error, since the
/// message is certainly still here and waits on the user.</param>
/// <param name="NoSubject">What stands in for a subject the message does not have. The same words
/// the message list and the reading header use, because it is the same absence.</param>
/// <remarks>
/// The words arrive as parameters because <c>L10n</c> needs a Windows TFM that Mailcal.Tests does
/// not have, and the glyphs because the Segoe Fluent codepoints live in the shell
/// (<c>MainWindow.Sidebar.cs</c> says why they are written as escapes).
/// </remarks>
public readonly record struct OutboxLabels(
    string Waiting,
    string Sending,
    string Unconfirmed,
    string NotSent,
    string WaitingGlyph,
    string SendingGlyph,
    string UnconfirmedGlyph,
    string NotSentGlyph,
    string NoSubject);

/// <summary>Projects the core's queued sends into the rows the Outbox list draws.</summary>
/// <remarks>
/// Internal, not public: it reads the generated <c>QueuedRow</c>, which UniFFI emits as an
/// internal type. <see cref="QueuedRowItem"/> stays public, because the XAML binds it.
/// </remarks>
internal static class OutboxRows
{
    /// <summary>
    /// The Outbox's rows, in the order the core gave them: oldest first, which is the order they
    /// will go out in.
    /// </summary>
    /// <param name="queued">The snapshot's queued sends.</param>
    /// <param name="labels">The state words and glyphs.</param>
    /// <param name="accountEmail">An account id resolved to the address the user knows it by. Every
    /// row draws it (docs/folder-pane.md, rule 18), because this list holds every account's mail at
    /// once and nothing else on the row says which one a message is waiting on.</param>
    public static List<QueuedRowItem> Build(
        IReadOnlyList<QueuedRow> queued,
        OutboxLabels labels,
        Func<string, string> accountEmail)
    {
        var rows = new List<QueuedRowItem>(queued.Count);
        foreach (var row in queued)
        {
            var state = StateOf(row.State);
            var account = accountEmail(row.Account);
            rows.Add(new QueuedRowItem
            {
                Account = row.Account,
                Op = row.Op,
                // Both lines can genuinely come back empty, and an empty line reads as a rendering
                // fault rather than as a fact. A blank subject is ordinary mail, which is why there
                // are already words for it; an empty To is a message addressed by Bcc alone, and
                // the account it goes from is then the only thing known about where it is going.
                ToText = row.To.Length > 0 ? row.To : account,
                SubjectText = row.Subject.Length > 0 ? row.Subject : labels.NoSubject,
                AccountText = account,
                State = state,
                StateText = TextFor(state, labels),
                StateGlyph = GlyphFor(state, labels),
                Actions = ActionsFor(state, row.Editable),
            });
        }
        return rows;
    }

    /// <summary>The mirror of the core's state onto the one the view models carry.</summary>
    public static QueuedSendState StateOf(QueuedState state) => state switch
    {
        QueuedState.Sending => QueuedSendState.Sending,
        QueuedState.Unconfirmed => QueuedSendState.Unconfirmed,
        QueuedState.NotSent => QueuedSendState.NotSent,
        _ => QueuedSendState.Waiting,
    };

    /// <summary>What a row in <paramref name="state"/> lists in its menu, in order.</summary>
    /// <param name="state">Where the send has got to.</param>
    /// <param name="editable">Whether a composer can hold the message. An invitation answer
    /// cannot, so its row leaves Edit out rather than offering an item that does nothing.</param>
    /// <remarks>
    /// An unconfirmed message may already be in front of its recipients, so it offers only the two
    /// answers and never Send now, Edit or Cancel: sending it again without the user saying it did
    /// not arrive is how it arrives twice. A message in flight lists Waiting's items, drawn
    /// disabled (<see cref="QueuedRowItem.IsActionable"/>), so its menu keeps the shape it had.
    /// </remarks>
    public static IReadOnlyList<QueuedAction> ActionsFor(QueuedSendState state, bool editable) => state switch
    {
        QueuedSendState.Unconfirmed => [QueuedAction.MarkSent, QueuedAction.ConfirmNotSent],
        QueuedSendState.NotSent when editable =>
            [QueuedAction.SendAgain, QueuedAction.Edit, QueuedAction.Discard],
        QueuedSendState.NotSent => [QueuedAction.SendAgain, QueuedAction.Discard],
        _ when editable => [QueuedAction.SendNow, QueuedAction.Edit, QueuedAction.Cancel],
        _ => [QueuedAction.SendNow, QueuedAction.Cancel],
    };

    /// <summary>Whether the item asks before its intent is sent.</summary>
    /// <remarks>
    /// Only sending an unconfirmed message again: the user is saying it did not arrive, and if
    /// they are wrong it arrives twice. Sending a refused one again asks nothing, because the
    /// server has said it did not go.
    /// </remarks>
    public static bool NeedsConfirming(QueuedAction action) => action is QueuedAction.ConfirmNotSent;

    /// <summary>Whether the item takes the message away for good, so the menu sets it apart.</summary>
    public static bool IsDestructive(QueuedAction action) =>
        action is QueuedAction.Cancel or QueuedAction.Discard;

    /// <summary>The intent an item sends for the queued send it names.</summary>
    /// <param name="action">The item chosen.</param>
    /// <param name="account">The account the send goes from.</param>
    /// <param name="op">The send's op id within that account's queue.</param>
    /// <param name="stagingDirectory">Where Edit has the core write the message's files, for the
    /// composer to open holding them. Every other item ignores it.</param>
    /// <remarks>
    /// Sending again is <c>SendNow</c> for a refused message and <c>ConfirmNotSent</c> for an
    /// unconfirmed one. The core refuses the second on any other state, so a row that changed
    /// under the click cannot send a message twice.
    /// </remarks>
    public static OutboxIntent IntentFor(
        QueuedAction action, string account, ulong op, string stagingDirectory) => action switch
    {
        QueuedAction.SendNow or QueuedAction.SendAgain => new OutboxIntent.SendNow(account, op),
        QueuedAction.Edit => new OutboxIntent.Edit(account, op, stagingDirectory),
        QueuedAction.Cancel or QueuedAction.Discard => new OutboxIntent.Cancel(account, op),
        QueuedAction.MarkSent => new OutboxIntent.ConfirmSent(account, op),
        QueuedAction.ConfirmNotSent => new OutboxIntent.ConfirmNotSent(account, op),
        _ => throw new ArgumentOutOfRangeException(nameof(action), action, null),
    };

    private static string TextFor(QueuedSendState state, OutboxLabels labels) => state switch
    {
        QueuedSendState.Sending => labels.Sending,
        QueuedSendState.Unconfirmed => labels.Unconfirmed,
        QueuedSendState.NotSent => labels.NotSent,
        _ => labels.Waiting,
    };

    private static string GlyphFor(QueuedSendState state, OutboxLabels labels) => state switch
    {
        QueuedSendState.Sending => labels.SendingGlyph,
        QueuedSendState.Unconfirmed => labels.UnconfirmedGlyph,
        QueuedSendState.NotSent => labels.NotSentGlyph,
        _ => labels.WaitingGlyph,
    };
}
