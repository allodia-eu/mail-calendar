// Public, render-ready reading-view types the XAML binds to, the open message's header
// context and its fetched body. Like the list-row view-models, these keep the generated
// (internal, lowercase-field) UniFFI snapshot types confined to the service layer:
// MailboxModel records the tapped row's header as an OpenedMessage and projects the FFI
// ReadingSnapshot into a ReadingBody. Mirrors the macOS OpenedMessage / ReadingSnapshot split.

namespace Allodia.Mailcal.ViewModels;

/// <summary>
/// The header context for an opened message (the row the user tapped). The body is fetched
/// separately into <see cref="ReadingBody"/> and matched back by <see cref="Key"/>.
/// </summary>
public sealed class OpenedMessage
{
    /// <summary>The id of the account that owns this message (so reply/forward route to it).</summary>
    public string Account { get; init; } = string.Empty;

    /// <summary>The message's provider key (matches its body snapshot).</summary>
    public required string Key { get; init; }

    /// <summary>The subject, with a placeholder when empty.</summary>
    public required string Subject { get; init; }

    /// <summary>The sender address.</summary>
    public required string From { get; init; }

    /// <summary>
    /// The sender's face as the list row already had it, so the header draws one immediately
    /// rather than flashing empty until the body snapshot arrives.
    /// </summary>
    public required AvatarItem Avatar { get; init; }

    /// <summary>The received date, already localised to the active display zone.</summary>
    public required string DateText { get; init; }
}

/// <summary>
/// An opened message's fetched, sanitised body, the projection of the core's
/// <c>ReadingSnapshot</c>. <see cref="Html"/> is the core-sanitized HTML (presentational CSS
/// kept; scripts/handlers/frames stripped); the reading view wraps it with the model's
/// <c>RenderMessageHtml</c> and loads it into a locked-down WebView2. <see cref="Plain"/> is
/// the fallback when the message has no HTML part.
/// </summary>
public sealed class ReadingBody
{
    /// <summary>The provider key of the message this body is for (matches the header).</summary>
    public required string Key { get; init; }

    /// <summary>
    /// The sender, formatted as <c>Name &lt;email&gt;</c> (or bare <c>email</c> when the header
    /// carried no name); empty when none. The reading header shows this full form, unlike the
    /// list row's name-only sender.
    /// </summary>
    public string From { get; init; } = string.Empty;

    /// <summary>
    /// The sender's face, as the core resolved it for this message.
    /// </summary>
    /// <remarks>
    /// Read from the core rather than derived here, which is the rule docs/avatars.md states for
    /// every surface built one at a time instead of as a list: a header that projected a fresh
    /// avatar would draw initials over a list row already showing the photograph. It queues no
    /// lookup of its own, this pane is always opened from a row that is still on screen beside
    /// it, so there is nothing left to ask.
    /// </remarks>
    public required AvatarItem Avatar { get; init; }

    /// <summary>The <c>To</c> recipients, formatted and comma-joined; empty when none.</summary>
    public string To { get; init; } = string.Empty;

    /// <summary>The <c>Cc</c> recipients, formatted and comma-joined; empty when none.</summary>
    public string Cc { get; init; } = string.Empty;

    /// <summary>
    /// The <c>Bcc</c> recipients, formatted and comma-joined; empty when none. Present only on
    /// the sender's own Sent/Drafts copy, so they can see whom they Bcc'd.
    /// </summary>
    public string Bcc { get; init; } = string.Empty;

    /// <summary>The sanitised HTML body, or null when the message has no HTML part.</summary>
    public string? Html { get; init; }

    /// <summary>The plain-text body, or null, the fallback when <see cref="Html"/> is null.</summary>
    public string? Plain { get; init; }

    /// <summary>Downloadable attachments decoded from the message source.</summary>
    public IReadOnlyList<MessageAttachment> Attachments { get; init; } = Array.Empty<MessageAttachment>();

    /// <summary>
    /// Whether the HTML references a remote resource blocked by default, the signal to offer
    /// a "load remote images" confirmation, then re-render with remote images on.
    /// </summary>
    public bool HasRemoteImages { get; init; }

    /// <summary>
    /// Whether the body could not be <em>fetched</em> (a provider/network error), as distinct
    /// from a message that genuinely has no body. The reading view shows a "couldn't load,
    /// retry" affordance for this, not the silent empty-state.
    /// </summary>
    public bool LoadError { get; init; }

    /// <summary>
    /// Whether the open for <see cref="Key"/> is still running and has lasted long enough to be
    /// worth saying so, the one signal that may raise the loading ring.
    /// </summary>
    /// <remarks>
    /// Never show a ring merely because no snapshot has arrived for the message being opened: a
    /// stored body comes back in milliseconds, so a ring on every open appears and vanishes
    /// inside an eyeblink and reads as flicker. Until this is set, draw the body area empty and
    /// let the header the list row already gave you carry the pane. The core times the wait, so
    /// every platform draws the same conclusion. A snapshot carrying this has no body, so read
    /// it before the branches that look for one.
    /// </remarks>
    public bool Pending { get; init; }

    /// <summary>
    /// The meeting-invitation card, when the core's two-condition RSVP gate says this message
    /// carries one (<c>docs/invitations.md</c>); <c>null</c> otherwise, and then the reading pane
    /// draws no card at all.
    /// </summary>
    /// <remarks>
    /// The one field here that is <b>not</b> re-projected into a Windows view-model, and
    /// deliberately: the card is a calendar surface, twenty fields plus a solved day of grid
    /// geometry, and copying it would be a second place for the numbers to drift from what the
    /// core decided. Same call the calendar layer already makes, where <c>MonthGridView</c> takes
    /// the generated <c>MonthPage</c> as it comes. Internal, so the generated type stays
    /// out of this assembly's public surface.
    /// </remarks>
    internal uniffi.mailcal_bindings.InvitationCard? Invitation { get; init; }
}

/// <summary>One downloadable attachment in the reading view.</summary>
public sealed class MessageAttachment
{
    /// <summary>The MIME part id used when asking the core to decode this attachment.</summary>
    public uint Id { get; init; }

    /// <summary>The display filename supplied by the message, sanitised in the core.</summary>
    public required string FileName { get; init; }

    /// <summary>The attachment media type.</summary>
    public required string MediaType { get; init; }

    /// <summary>The decoded attachment byte size.</summary>
    public ulong Size { get; init; }

    /// <summary>Compact detail text for the list row.</summary>
    public string Detail => $"{MediaType} · {FormatBytes(Size)}";

    private static string FormatBytes(ulong bytes)
    {
        string[] units = ["B", "KB", "MB", "GB"];
        var value = (double)bytes;
        var unit = 0;
        while (value >= 1024 && unit < units.Length - 1)
        {
            value /= 1024;
            unit++;
        }
        return unit == 0 ? $"{bytes} {units[unit]}" : $"{value:0.#} {units[unit]}";
    }
}
