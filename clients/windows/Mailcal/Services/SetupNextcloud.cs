// What setup decides about a sign-in with Nextcloud (docs/nextcloud-sign-in.md rule 2): which
// server and uses a press on the found card or the manual form signs in at, and whether the manual
// calendar and contacts form offers it. WinUI-free and L10n-free, so Mailcal.Tests drives it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Where a press signs in, and what the account it makes is used for.</summary>
internal sealed record NextcloudStart(string Server, AccountCapability[] Uses);

/// <summary>
/// The manual calendar and contacts form's question to its server: asked once per server, when a
/// server field is left, and drawn only while the form still names the server that said yes.
/// </summary>
internal sealed class SetupNextcloudOffer
{
    private string? _asked;
    private string? _offered;

    /// <summary>The server a manual form names: its calendar server, else its address book's.
    /// Null when it names neither.</summary>
    internal static string? ManualServer(string caldav, string carddav) =>
        new[] { caldav.Trim(), carddav.Trim() }.FirstOrDefault(url => url.Length > 0);

    /// <summary>A server field was left: the server to ask about, or null when the form names none
    /// or the one it names was asked already.</summary>
    internal string? Ask(string caldav, string carddav)
    {
        if (ManualServer(caldav, carddav) is not { } server || server == _asked)
        {
            return null;
        }
        _asked = server;
        return server;
    }

    /// <summary>The server answered. Only the answer for the server last asked counts.</summary>
    internal void Answered(string server, bool offers)
    {
        if (server != _asked)
        {
            return;
        }
        _offered = offers ? server : null;
    }

    /// <summary>The server the form offers the sign-in at, for what it names now; null while it
    /// names another, or the one it names is not Nextcloud.</summary>
    internal string? OfferedFor(string caldav, string carddav) =>
        _offered is { } offered && ManualServer(caldav, carddav) == offered ? offered : null;

    /// <summary><paramref name="server"/> is known to be Nextcloud already: detection asked, and the
    /// form was opened from its card.</summary>
    internal void Known(string server)
    {
        _asked = server;
        _offered = server;
    }

    /// <summary>A form opened again starts from nothing asked.</summary>
    internal void Reset()
    {
        _asked = null;
        _offered = null;
    }

    /// <summary>The found card's press: at the calendar's server, else the address book's, for the
    /// uses chosen, or those the servers decide when the card offered no choice. Null when it names
    /// no server.</summary>
    internal static NextcloudStart? Found(ChosenUses chosen)
    {
        if (ManualServer(chosen.CaldavUrl, chosen.CarddavUrl) is not { } server)
        {
            return null;
        }
        return (chosen.Uses ?? SetupUses.DavUses(chosen.CaldavUrl, chosen.CarddavUrl)) is { } uses
            ? new NextcloudStart(server, uses)
            : null;
    }

    /// <summary>The manual form's press, at <paramref name="server"/>, the one that answered: the
    /// account is used for what the servers typed make it.</summary>
    internal static NextcloudStart? Manual(string server, string caldav, string carddav) =>
        SetupUses.DavUses(caldav.Trim(), carddav.Trim()) is { } uses ? new NextcloudStart(server, uses) : null;
}
