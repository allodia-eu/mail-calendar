// An account's servers and sign-in as Settings edits them (docs/accounts.md rule 14): what the
// fields show for a stored address, and what is sent back. WinUI-free so the rules are reachable
// from Mailcal.Tests; the core tries the servers before it keeps them, so a typo never costs a
// working account.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The fields as the person left them; null for a server the form drew no field for.</summary>
internal sealed record EndpointFields(
    string? ImapHost,
    ConnectionSecurity? ImapSecurity,
    string? SmtpHost,
    ConnectionSecurity? SmtpSecurity,
    string? CaldavUrl,
    string? CarddavUrl,
    string Username,
    string Password);

internal static class AccountEndpointEdit
{
    /// <summary>
    /// The servers to save: <paramref name="original"/> with what the fields changed. A server the
    /// account does not have stays absent, one with no field keeps its value, and an empty
    /// password keeps the stored one.
    /// </summary>
    internal static AccountEndpoints Edited(AccountEndpoints original, EndpointFields fields)
    {
        static string? Kept(string? old, string? typed) => old is null ? null : typed ?? old;
        return new AccountEndpoints(
            ImapHost: Kept(original.ImapHost, fields.ImapHost),
            ImapSecurity: fields.ImapSecurity ?? original.ImapSecurity,
            SmtpHost: Kept(original.SmtpHost, fields.SmtpHost),
            SmtpSecurity: fields.SmtpSecurity ?? original.SmtpSecurity,
            CaldavUrl: Kept(original.CaldavUrl, fields.CaldavUrl),
            CarddavUrl: Kept(original.CarddavUrl, fields.CarddavUrl),
            Username: fields.Username,
            Password: fields.Password.Length == 0 ? null : fields.Password);
    }

    /// <summary>
    /// The address to store, joined as the core joins it: the bare host when the port is empty or
    /// the standard one for <paramref name="security"/>, and <c>host:port</c> when it is not, a
    /// port the person wrote into the host winning over the port field, as the setup form dials
    /// it. No host is no address, whatever the port says, so the core sees the server as missing.
    /// </summary>
    internal static string Joined(
        string host,
        string port,
        ConnectionSecurity security,
        Func<ConnectionSecurity, ushort> standardPort,
        ServerAddressing addressing)
    {
        var (bareHost, inHost) = addressing.Split(host);
        if (bareHost.Length == 0)
        {
            return string.Empty;
        }
        var chosen = inHost.Length > 0 ? inHost : port.Trim();
        return chosen.Length == 0 || chosen == Port(standardPort(security))
            ? bareHost
            : addressing.Join(bareHost, chosen);
    }

    private static string Port(ushort port) =>
        port.ToString(System.Globalization.CultureInfo.InvariantCulture);
}
