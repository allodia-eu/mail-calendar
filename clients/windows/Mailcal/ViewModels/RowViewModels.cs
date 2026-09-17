// Public, render-ready row types the XAML binds to. The generated UniFFI snapshot types
// are `internal` (and carry lowercase Rust field names), so MailboxModel projects each
// snapshot into these public POCOs, keeping the FFI types confined to the service layer
// and the data-binding clean. Display-only visibility/weight helpers live here so the XAML
// stays declarative (no converters needed), mirroring how the macOS views compute glyphs.

using System;
using System.Collections.Generic;
using System.ComponentModel;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.ViewModels;

// FolderItem and AccountItem, the two the sidebar reads, live in SidebarSources.cs, so that
// SidebarTree can be linked into Mailcal.Tests without dragging this file's WinUI types along.
// MailRow is in MailRow.cs, because the change notifications it carries are worth reading whole.


/// <summary>
/// One message within an expanded conversation (a thread sub-row): who sent it, when, a preview,
/// and the display flags (unread, the "Sent" badge, an attachment). <see cref="Key"/> opens it.
/// </summary>
public sealed class ThreadMessageItem
{
    /// <summary>The id of the account this message belongs to.</summary>
    public required string Account { get; init; }

    /// <summary>The message's provider key (what a tap opens for reading).</summary>
    public required string Key { get; init; }

    /// <summary>The conversation's subject, the header shown when this message opens.</summary>
    public required string Subject { get; init; }

    /// <summary>The subject exactly as the conversation carries it; see MailRow.RawSubject.</summary>
    public string RawSubject { get; init; } = string.Empty;

    /// <summary>The sender address, with a placeholder when empty.</summary>
    public required string FromText { get; init; }

    /// <summary>The sender's face, this message's own, not the conversation's.</summary>
    public required AvatarItem Avatar { get; init; }

    /// <summary>The message's date as a compact relative label (the sub-row's date).</summary>
    public required string DateText { get; init; }

    /// <summary>
    /// The message's date as a full absolute timestamp, what the reading header shows when this
    /// sub-row is opened, as opposed to <see cref="DateText"/>'s relative label. See
    /// docs/timestamps.md.
    /// </summary>
    public required string FullDateText { get; init; }

    /// <summary>A short preview snippet (empty if none).</summary>
    public required string PreviewText { get; init; }

    /// <summary>Whether the message is unread.</summary>
    public bool Unread { get; init; }

    /// <summary>Whether the account owner sent it (drives the "Sent" badge).</summary>
    public bool Outgoing { get; init; }

    /// <summary>Whether the message has a non-inline attachment.</summary>
    public bool HasAttachment { get; init; }

    /// <summary>Show the unread accent dot only on unread messages.</summary>
    public Visibility UnreadVisibility => Unread ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Bold an unread sender, like the macOS thread sub-row.</summary>
    public Windows.UI.Text.FontWeight FromWeight =>
        Unread ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Normal;

    /// <summary>Show the "Sent" badge only on the owner's own messages.</summary>
    public Visibility SentVisibility => Outgoing ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>The "Sent" badge label.</summary>
    public string SentText => L10n.ThreadSent();

    /// <summary>Show the preview line only when there is one.</summary>
    public Visibility PreviewVisibility =>
        string.IsNullOrEmpty(PreviewText) ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>Show the paperclip only when the message carries an attachment.</summary>
    public Visibility AttachmentVisibility => HasAttachment ? Visibility.Visible : Visibility.Collapsed;
}

/// <summary>One agenda row: the event's key, title, and localised start.</summary>
public sealed class EventItem
{
    /// <summary>The id of the account this event belongs to (so a delete routes to it).</summary>
    public string Account { get; init; } = string.Empty;

    /// <summary>The event's provider key (used to delete it).</summary>
    public required string Key { get; init; }

    /// <summary>The event's title, with a placeholder when empty.</summary>
    public required string Title { get; init; }

    /// <summary>The start, already localised to the active display zone.</summary>
    public required string StartText { get; init; }

    /// <summary>
    /// Whether this row offers a delete, the projection's CalendarWriteGating decision over the
    /// core's write flag on this exact record.
    /// </summary>
    public bool OffersDelete { get; init; }

    /// <summary>Show the delete affordances only on writable rows (hidden, not disabled, a
    /// disabled delete on a read-only calendar is just a mystery).</summary>
    public Visibility DeleteVisibility => OffersDelete ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// "Awaiting your response" on an unanswered invitation, empty otherwise.
    /// </summary>
    /// <remarks>
    /// A list row has no border to dash and no gutter to hatch, so the hold says itself in words,
    /// which is the disclosure the dashes only ever stood in for (docs/calendar.md §4,
    /// docs/invitations.md). The grid and the month chip draw it and <i>also</i> say it; here the
    /// words are the whole treatment.
    /// </remarks>
    public string AwaitingText { get; init; } = string.Empty;

    /// <summary>Show the hold line only on a hold, never an empty row of chrome.</summary>
    public Visibility AwaitingVisibility =>
        AwaitingText.Length > 0 ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// What a screen reader announces for this row: title, start, then the awaiting-response line
    /// when there is one.
    /// </summary>
    /// <remarks>
    /// The same <c>ListViewItem</c> fallback <see cref="MailRow.AccessibleName"/> exists for, in
    /// the other list this app draws: with no UIA Name of its own the peer reads
    /// <c>ToString()</c>, so every agenda row announced "Allodia.Mailcal.ViewModels.EventItem"
    /// until this existed, one phrase for the whole day, with no title and no time.
    ///
    /// The hold is appended rather than woven in, for the reason the mail row appends its state
    /// words: <see cref="AwaitingText"/> is already localised, so no new phrase needs translating
    /// per combination.
    /// </remarks>
    public string AccessibleName
    {
        get
        {
            var text = L10n.A11yEventRow(Title, StartText);
            if (AwaitingText.Length > 0)
            {
                text += ", " + AwaitingText;
            }
            return text;
        }
    }

    /// <summary>The row's spoken label, see <see cref="AccessibleName"/> for why this override.</summary>
    public override string ToString() => AccessibleName;
}
