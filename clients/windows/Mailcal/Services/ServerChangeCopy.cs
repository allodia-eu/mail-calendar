// What the question about a calendar or address book server found on another site says
// (docs/account-autodetect.md rule 14, docs/accounts.md rule 16). WinUI-free and L10n-free, like
// AccountSettingsRules: the words arrive as ServerChangeWords, so which title, body and closing
// line a change earns, and where each site sits in its line, are tested in Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which uses a server change moves, and so which variant of each sentence it gets.</summary>
internal enum ServerChangeUses
{
    Calendar,
    Contacts,
    Both,
}

/// <summary>The words the question is made of.</summary>
/// <param name="Title">"Your calendar server has changed", per use.</param>
/// <param name="Body">"The calendar for {address} now points to a different server:", per use.</param>
/// <param name="Before">"Before: {site}".</param>
/// <param name="Now">"Now: {site}".</param>
/// <param name="Unsure">"If you're not sure, choose "Not now"…", per use.</param>
internal sealed record ServerChangeWords(
    Func<ServerChangeUses, string> Title,
    Func<ServerChangeUses, string, string> Body,
    Func<string, string> Before,
    Func<string, string> Now,
    string Advice,
    Func<ServerChangeUses, string> Unsure);

/// <summary>A line naming a site, split around it so the site can be drawn in bold.</summary>
internal sealed record SiteLine(string Lead, string Site, string Tail);

/// <summary>The question for one change, sentence by sentence.</summary>
internal sealed record ServerChangeCopy(
    string Title, string Body, SiteLine Before, SiteLine Now, string Advice, string Unsure)
{
    /// <summary>Which uses <paramref name="change"/> moves. A change that names neither is read as
    /// the calendar: the core never sends one.</summary>
    internal static ServerChangeUses Uses(ServerChange change) => (change.Calendar, change.Contacts) switch
    {
        (true, true) => ServerChangeUses.Both,
        (false, true) => ServerChangeUses.Contacts,
        _ => ServerChangeUses.Calendar,
    };

    /// <summary>The question for <paramref name="change"/> on the account at
    /// <paramref name="address"/>.</summary>
    internal static ServerChangeCopy Of(ServerChange change, string address, ServerChangeWords words)
    {
        var uses = Uses(change);
        return new ServerChangeCopy(
            words.Title(uses),
            words.Body(uses, address),
            Around(words.Before, change.PreviousSite),
            Around(words.Now, change.ProposedSite),
            words.Advice,
            words.Unsure(uses));
    }

    /// <summary>
    /// <paramref name="line"/> said with <paramref name="site"/>, split where the site goes. The
    /// split is found with a marker rather than by searching for the site, which a translation's own
    /// words could contain; a translation without the placeholder leaves the site after it.
    /// </summary>
    internal static SiteLine Around(Func<string, string> line, string site)
    {
        const string marker = "\u0000";
        var said = line(marker);
        var at = said.IndexOf(marker, StringComparison.Ordinal);
        return at < 0
            ? new SiteLine(said + " ", site, string.Empty)
            : new SiteLine(said[..at], site, said[(at + marker.Length)..]);
    }
}
