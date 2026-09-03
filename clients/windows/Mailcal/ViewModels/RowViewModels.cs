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

/// <summary>
/// One mailbox-list row, a single message (flat) or a conversation (threaded), flattened
/// for display. <see cref="Key"/> is the message key (flat) or thread id (threaded).
/// </summary>
public sealed class MailRow : INotifyPropertyChanged
{
    /// <summary>Stable list identity ("m:&lt;key&gt;" / "t:&lt;thread&gt;") for diffing.</summary>
    public required string Id { get; init; }

    /// <summary>The id of the account this row belongs to (which inbox it came from).</summary>
    public string Account { get; init; } = string.Empty;

    /// <summary>Whether this row is a conversation summary rather than a single message.</summary>
    public bool IsThread { get; init; }

    /// <summary>The message's provider key (flat) or the thread id (threaded).</summary>
    public required string Key { get; init; }

    /// <summary>
    /// The provider key of the message a tap opens for reading: the message key for a flat
    /// row, the thread's latest message key for a conversation row (a thread has no key of
    /// its own). So the reading view works in threaded mode too.
    /// </summary>
    public required string LatestKey { get; init; }

    /// <summary>The subject exactly as the message carries it, empty and all. What the row
    /// SHOWS is <see cref="Title"/>, which substitutes a placeholder; a composer must not open
    /// with "Re: (no subject)" in a field the user is about to send.</summary>
    public string RawSubject { get; init; } = string.Empty;

    /// <summary>The subject, with a placeholder when empty.</summary>
    public required string Title { get; init; }

    /// <summary>The sender address (latest sender for a thread).</summary>
    public required string Subtitle { get; init; }

    /// <summary>
    /// The sender's face: their photo where an address book has one, else their monogram
    /// (docs/avatars.md). A thread row carries its latest sender's, which is who the rest of the
    /// row names.
    /// </summary>
    public required AvatarItem Avatar { get; init; }

    /// <summary>The received date as a compact relative label (the list row's date).</summary>
    public required string DateText { get; init; }

    /// <summary>
    /// The received date as a full absolute timestamp, the date the reading header shows when this
    /// row is opened, as opposed to <see cref="DateText"/>'s compact relative label. See
    /// docs/timestamps.md.
    /// </summary>
    public required string FullDateText { get; init; }

    /// <summary>
    /// Whether the row is unread: the message itself (flat), or any message in the conversation
    /// (threaded). A conversation summary may not read as settled while it hides an unread reply.
    /// </summary>
    public bool Unread { get; init; }

    /// <summary>Whether the message is flagged (flat rows only).</summary>
    public bool Flagged { get; init; }

    /// <summary>
    /// Whether the row carries an attachment, a single message with one (flat), or any message
    /// in the conversation (threaded). Drives the paperclip indicator, mirroring the macOS list.
    /// </summary>
    public bool HasAttachment { get; init; }

    /// <summary>How many messages a thread holds (threaded rows).</summary>
    public uint MessageCount { get; init; }

    /// <summary>Bold the subject of an unread message, like the macOS and Android lists.</summary>
    public Windows.UI.Text.FontWeight TitleWeight =>
        Unread ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Normal;

    /// <summary>
    /// Bold the sender of an unread row too. Subject and sender move together on every platform:
    /// the accent dot alone is a small target for the eye when scanning a full mailbox, and it is
    /// the sender that tells you whether an unread row is worth opening.
    /// </summary>
    public Windows.UI.Text.FontWeight SubtitleWeight =>
        Unread ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Normal;

    /// <summary>Show the unread accent dot only on unread rows.</summary>
    public Visibility UnreadVisibility => Unread ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Show the flag indicator only on flagged rows.</summary>
    public Visibility FlagVisibility => Flagged ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Show the paperclip only when the row carries an attachment (mirrors the macOS list).</summary>
    public Visibility AttachmentVisibility => HasAttachment ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Show the count badge only for multi-message threads.</summary>
    public Visibility CountVisibility =>
        IsThread && MessageCount > 1 ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>The count-badge text.</summary>
    public string CountText => MessageCount.ToString();

    /// <summary>The context-menu label for the read/unread toggle.</summary>
    public string MarkReadText => Unread ? L10n.ActionMarkRead() : L10n.ActionMarkUnread();

    /// <summary>The context-menu label for the flag toggle.</summary>
    public string FlagToggleText => Flagged ? L10n.ActionClearFlag() : L10n.ActionFlag();

    /// <summary>
    /// The whole conversation, newest first (empty for a flat row), the sub-rows an expanded
    /// thread reveals. The first entry is the message at <see cref="LatestKey"/>.
    /// </summary>
    public IReadOnlyList<ThreadMessageItem> Messages { get; init; } = Array.Empty<ThreadMessageItem>();

    private bool _isExpanded;

    /// <summary>
    /// Whether this conversation is expanded inline (its sub-rows shown). Mutable UI state
    /// (unlike the projected fields), so it raises change notifications; the projection restores
    /// it from the model's remembered set after a refresh so a sync doesn't collapse open threads.
    /// </summary>
    public bool IsExpanded
    {
        get => _isExpanded;
        set
        {
            if (_isExpanded == value)
            {
                return;
            }
            _isExpanded = value;
            OnPropertyChanged(nameof(IsExpanded));
            OnPropertyChanged(nameof(SubRowsVisibility));
            OnPropertyChanged(nameof(ChevronGlyph));
        }
    }

    /// <summary>Show the expanded sub-rows only while expanded.</summary>
    public Visibility SubRowsVisibility => IsExpanded ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>The expand/collapse chevron shows on conversation rows (a thread can expand).</summary>
    public Visibility ChevronVisibility => IsThread ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>The chevron glyph: down when expanded, right when collapsed.</summary>
    public string ChevronGlyph => IsExpanded ? "" : "";

    /// <summary>
    /// What a screen reader announces for this row: sender, subject, date, then whichever of
    /// unread / flagged / has-attachment apply.
    /// </summary>
    /// <remarks>
    /// A <c>ListViewItem</c> whose content comes from a DataTemplate has no UIA Name of its own, so
    /// its peer falls back to <c>ToString()</c> on the bound object, which announced
    /// "Allodia.Mailcal.ViewModels.MailRow" on every row until this existed. That is why
    /// <see cref="ToString"/> below returns this: the fallback IS the announcement, so the fix has
    /// to live where the fallback looks. Nothing about the visual row reveals the defect, and no
    /// headless gate can see it, <c>uitests/Accessibility.Tests.ps1</c> is what watches it now.
    ///
    /// The three state words are appended rather than woven in: each already exists in the catalog,
    /// so a row stays fully localised without a new phrase to translate per state combination.
    /// </remarks>
    public string AccessibleName
    {
        get
        {
            var text = L10n.A11yMailRow(Subtitle, Title, DateText);
            if (Unread)
            {
                text += ", " + L10n.ActionUnread();
            }
            if (Flagged)
            {
                text += ", " + L10n.A11yFlagged();
            }
            if (HasAttachment)
            {
                text += ", " + L10n.A11yHasAttachment();
            }
            return text;
        }
    }

    /// <summary>The row's spoken label, see <see cref="AccessibleName"/> for why this override.</summary>
    public override string ToString() => AccessibleName;

    /// <inheritdoc/>
    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged(string name) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}

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
