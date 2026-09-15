// One mailbox-list row. Split out of RowViewModels.cs because this type carries the change
// notifications the whole list depends on, and they are worth reading as one thing.
//
// WHY THE PROJECTED FIELDS NOTIFY. A row used to be immutable: the projection built a new MailRow
// and the reconcile swapped it into the collection, which is why every binding in the row template
// could be OneTime. Swapping the item makes the ListView throw the old container away and build a
// new one, and WinUI 3 reassigns focus when the element holding it is removed, which ACTIVATES the
// window that element was in (microsoft-ui-xaml#8520). So a reading window opened by double-click
// sank behind the mailbox about a second later: opening the message marked it read, the row's
// Unread changed, the row the double-click had left focused was replaced, and the mailbox took the
// foreground back.
//
// So the projected fields are settable, through CopyFrom, and the row template binds them OneWay.
// The instance outlives a content change, its container with it, and nothing reassigns focus.
// Construction is unchanged: every field is still `init`, so only this class can move one.

using System;
using System.Collections.Generic;
using System.ComponentModel;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.ViewModels;

/// <summary>
/// One mailbox-list row, a single message (flat) or a conversation (threaded), flattened
/// for display. <see cref="Key"/> is the message key (flat) or thread id (threaded).
/// </summary>
public sealed class MailRow : INotifyPropertyChanged, Allodia.Mailcal.Services.ISelectableRow
{
    /// <summary>Stable list identity ("m:&lt;key&gt;" / "t:&lt;thread&gt;") for diffing.</summary>
    public required string Id { get; init; }

    /// <summary>The id of the account this row belongs to (which inbox it came from).</summary>
    public string Account { get; init; } = string.Empty;

    /// <summary>Whether this row is a conversation summary rather than a single message.</summary>
    public bool IsThread { get; init; }

    /// <summary>The message's provider key (flat) or the thread id (threaded).</summary>
    public required string Key { get; init; }

    private string _latestKey = string.Empty;

    /// <summary>
    /// The provider key of the message a tap opens for reading: the message key for a flat
    /// row, the thread's latest message key for a conversation row (a thread has no key of
    /// its own). So the reading view works in threaded mode too.
    /// </summary>
    public required string LatestKey { get => _latestKey; init => _latestKey = value; }

    private string _rawSubject = string.Empty;

    /// <summary>The subject exactly as the message carries it, empty and all. What the row
    /// SHOWS is <see cref="Title"/>, which substitutes a placeholder; a composer must not open
    /// with "Re: (no subject)" in a field the user is about to send.</summary>
    public string RawSubject { get => _rawSubject; init => _rawSubject = value; }

    private string _title = string.Empty;

    /// <summary>The subject, with a placeholder when empty.</summary>
    public required string Title { get => _title; init => _title = value; }

    private string _subtitle = string.Empty;

    /// <summary>The sender address (latest sender for a thread).</summary>
    public required string Subtitle { get => _subtitle; init => _subtitle = value; }

    // null-forgiving because the property is `required`: nothing can observe this before the
    // object initialiser has run, and AvatarItem is deliberately not publicly constructible.
    private AvatarItem _avatar = null!;

    /// <summary>
    /// The sender's face: their photo where an address book has one, else their monogram
    /// (docs/avatars.md). A thread row carries its latest sender's, which is who the rest of the
    /// row names.
    /// </summary>
    public required AvatarItem Avatar { get => _avatar; init => _avatar = value; }

    private string _dateText = string.Empty;

    /// <summary>The received date as a compact relative label (the list row's date).</summary>
    public required string DateText { get => _dateText; init => _dateText = value; }

    private string _fullDateText = string.Empty;

    /// <summary>
    /// The received date as a full absolute timestamp, the date the reading header shows when this
    /// row is opened, as opposed to <see cref="DateText"/>'s compact relative label. See
    /// docs/timestamps.md.
    /// </summary>
    public required string FullDateText { get => _fullDateText; init => _fullDateText = value; }

    private bool _unread;

    /// <summary>
    /// Whether the row is unread: the message itself (flat), or any message in the conversation
    /// (threaded). A conversation summary may not read as settled while it hides an unread reply.
    /// </summary>
    public bool Unread { get => _unread; init => _unread = value; }

    private bool _flagged;

    /// <summary>Whether the message is flagged (flat rows only).</summary>
    public bool Flagged { get => _flagged; init => _flagged = value; }

    private bool _hasAttachment;

    /// <summary>
    /// Whether the row carries an attachment, a single message with one (flat), or any message
    /// in the conversation (threaded). Drives the paperclip indicator, mirroring the macOS list.
    /// </summary>
    public bool HasAttachment { get => _hasAttachment; init => _hasAttachment = value; }

    private uint _messageCount;

    /// <summary>How many messages a thread holds (threaded rows).</summary>
    public uint MessageCount { get => _messageCount; init => _messageCount = value; }

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

    private IReadOnlyList<ThreadMessageItem> _messages = Array.Empty<ThreadMessageItem>();

    /// <summary>
    /// The whole conversation, newest first (empty for a flat row), the sub-rows an expanded
    /// thread reveals. The first entry is the message at <see cref="LatestKey"/>.
    /// </summary>
    public IReadOnlyList<ThreadMessageItem> Messages { get => _messages; init => _messages = value; }

    /// <summary>
    /// Whether two rows carry the same sub-row faces.
    /// </summary>
    /// <remarks>
    /// A thread's own avatar is its LATEST sender's, so an earlier sender's photo arriving in a
    /// second snapshot moves nothing on the header row while the expanded conversation under it
    /// still shows initials. The one rule, here rather than beside the projection, because
    /// <see cref="CopyFrom"/> has to make the same judgement the diff did.
    /// </remarks>
    public static bool SameFaces(
        IReadOnlyList<ThreadMessageItem> a, IReadOnlyList<ThreadMessageItem> b)
    {
        if (a.Count != b.Count)
        {
            return false;
        }
        for (var index = 0; index < a.Count; index++)
        {
            if (a[index].Avatar != b[index].Avatar)
            {
                return false;
            }
        }
        return true;
    }

    /// <summary>
    /// Takes every projected field from <paramref name="next"/>, announcing the ones that moved.
    /// </summary>
    /// <remarks>
    /// The alternative is replacing this instance in the bound collection, which destroys the
    /// row's container: see this file's header for what that costs. Only the fields the projection
    /// produces are copied; <see cref="IsExpanded"/> is the view's own state and survives, which is
    /// what keeps a sync from collapsing an open thread.
    /// </remarks>
    internal void CopyFrom(MailRow next)
    {
        _latestKey = next.LatestKey;
        _rawSubject = next.RawSubject;
        _fullDateText = next.FullDateText;
        if (_title != next.Title)
        {
            _title = next.Title;
            Changed(nameof(Title), nameof(AccessibleName));
        }
        if (_subtitle != next.Subtitle)
        {
            _subtitle = next.Subtitle;
            Changed(nameof(Subtitle), nameof(AccessibleName));
        }
        if (_dateText != next.DateText)
        {
            _dateText = next.DateText;
            Changed(nameof(DateText), nameof(AccessibleName));
        }
        if (_avatar != next.Avatar)
        {
            _avatar = next.Avatar;
            Changed(nameof(Avatar));
        }
        if (_unread != next.Unread)
        {
            _unread = next.Unread;
            Changed(
                nameof(Unread), nameof(TitleWeight), nameof(SubtitleWeight),
                nameof(UnreadVisibility), nameof(MarkReadText), nameof(AccessibleName));
        }
        if (_flagged != next.Flagged)
        {
            _flagged = next.Flagged;
            Changed(
                nameof(Flagged), nameof(FlagVisibility), nameof(FlagToggleText),
                nameof(AccessibleName));
        }
        if (_hasAttachment != next.HasAttachment)
        {
            _hasAttachment = next.HasAttachment;
            Changed(
                nameof(HasAttachment), nameof(AttachmentVisibility), nameof(AccessibleName));
        }
        if (_messageCount != next.MessageCount)
        {
            _messageCount = next.MessageCount;
            Changed(nameof(MessageCount), nameof(CountVisibility), nameof(CountText));
        }
        if (!SameFaces(_messages, next.Messages))
        {
            _messages = next.Messages;
            Changed(nameof(Messages));
        }
    }

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
            Changed(nameof(IsExpanded), nameof(SubRowsVisibility), nameof(ChevronGlyph));
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

    private void Changed(params string[] names)
    {
        foreach (var name in names)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }
    }
}
