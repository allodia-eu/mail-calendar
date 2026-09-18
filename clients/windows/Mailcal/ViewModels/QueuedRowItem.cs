// One unsent message, as the Outbox list draws it (docs/sending.md).
//
// Its own row type rather than a MailRow: a queued send is not a stored message. It has no
// provider key (nothing on a server holds it yet), no sender to show (it is the user), no read or
// flagged state, and no date it arrived. What it has instead is a reason it is still here, which
// no mail row carries.
//
// WinUI-free on purpose, like SidebarItem beside it: it is linked into Mailcal.Tests, a plain
// net10.0 assembly, so the rules in OutboxRows can be pinned without a window. That is also why
// the state's word and glyph arrive already resolved, as strings.

namespace Allodia.Mailcal.ViewModels;

/// <summary>
/// Where a queued send has got to, mirroring the core's <c>QueuedState</c> one-for-one.
/// </summary>
/// <remarks>
/// A mirror rather than the generated enum itself, for the reason <see cref="SidebarFolderRole"/>
/// is one: the UniFFI bindings are emitted <c>internal</c>, so a public member here cannot name
/// one. The mapping lives in <c>OutboxRows</c>, on the side of the line where they are visible.
/// </remarks>
public enum QueuedSendState
{
    /// <summary>Waiting to go: queued, or backing off between attempts.</summary>
    Waiting,

    /// <summary>Being sent right now.</summary>
    Sending,

    /// <summary>It may or may not have been delivered, and nothing will retry it by itself.</summary>
    Unconfirmed,
}

/// <summary>One message in the Outbox: who it is for, what it says, and why it has not gone.</summary>
public sealed class QueuedRowItem
{
    /// <summary>The id of the account it will be sent from.</summary>
    public required string Account { get; init; }

    /// <summary>
    /// The queued send's op id, which an action names together with <see cref="Account"/>.
    /// </summary>
    /// <remarks>
    /// Never on its own: an op id is unique only within its own account's queue, and this list
    /// holds every account's at once (docs/sending.md).
    /// </remarks>
    public required ulong Op { get; init; }

    /// <summary>
    /// The row's stable identity, for the snapshot reconcile: the account and the op together,
    /// for the reason <see cref="Op"/> gives.
    /// </summary>
    public string Id => $"q:{Account}:{Op}";

    /// <summary>The recipients, comma-joined, as the core composed them.</summary>
    public required string ToText { get; init; }

    /// <summary>The subject, or "(no subject)" when the message has none.</summary>
    public required string SubjectText { get; init; }

    /// <summary>
    /// The address of the account it will go out from, drawn on every row.
    /// </summary>
    /// <remarks>
    /// This list is the one place in the app that holds every account's mail at once
    /// (docs/folder-pane.md, rule 18), so without it a two-account user cannot tell which identity
    /// a stuck message is waiting on, which is usually the whole of why it is stuck.
    /// </remarks>
    public required string AccountText { get; init; }

    /// <summary>Where the send has got to.</summary>
    public QueuedSendState State { get; init; }

    /// <summary>The localised sentence for <see cref="State"/>, resolved by the projection.</summary>
    public required string StateText { get; init; }

    /// <summary>The Segoe Fluent glyph for <see cref="State"/>.</summary>
    public required string StateGlyph { get; init; }

    /// <summary>
    /// Whether this row's three actions are offered at all.
    /// </summary>
    /// <remarks>
    /// A message in flight is on its way and cannot be called back; one awaiting confirmation may
    /// already be in front of its recipients, and offering to send that again is how it arrives
    /// twice (docs/sending.md). Both show their state and offer nothing.
    /// </remarks>
    public bool IsActionable => State == QueuedSendState.Waiting;

    /// <summary>What a screen reader reads for the whole row, as one sentence.</summary>
    /// <remarks>
    /// Built here rather than left to the three TextBlocks: read separately they are announced as
    /// three items, which is what a list of messages sounds like, and this is not one.
    /// </remarks>
    public string A11yText => $"{ToText}. {SubjectText}. {StateText}. {AccountText}";
}
