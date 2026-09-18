// The Outbox list's one decision: what each queued send says about itself (docs/sending.md).
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
/// <param name="WaitingGlyph">The Segoe Fluent glyph for <paramref name="Waiting"/>.</param>
/// <param name="SendingGlyph">The glyph for <paramref name="Sending"/>.</param>
/// <param name="UnconfirmedGlyph">The glyph for <paramref name="Unconfirmed"/>. The only warning
/// of the three: it is the one state a person may need to check on another device.</param>
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
    string WaitingGlyph,
    string SendingGlyph,
    string UnconfirmedGlyph,
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
            });
        }
        return rows;
    }

    /// <summary>The mirror of the core's state onto the one the view models carry.</summary>
    public static QueuedSendState StateOf(QueuedState state) => state switch
    {
        QueuedState.Sending => QueuedSendState.Sending,
        QueuedState.Unconfirmed => QueuedSendState.Unconfirmed,
        // Waiting, and anything a later core adds: the plain, non-alarming reading, and the only
        // one of the three that offers the actions. A new state arriving here as actionable would
        // be worse than arriving as waiting, so the discard falls this way deliberately.
        _ => QueuedSendState.Waiting,
    };

    private static string TextFor(QueuedSendState state, OutboxLabels labels) => state switch
    {
        QueuedSendState.Sending => labels.Sending,
        QueuedSendState.Unconfirmed => labels.Unconfirmed,
        _ => labels.Waiting,
    };

    private static string GlyphFor(QueuedSendState state, OutboxLabels labels) => state switch
    {
        QueuedSendState.Sending => labels.SendingGlyph,
        QueuedSendState.Unconfirmed => labels.UnconfirmedGlyph,
        _ => labels.WaitingGlyph,
    };
}
