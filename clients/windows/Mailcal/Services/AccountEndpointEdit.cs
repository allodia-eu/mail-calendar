// An account's servers and sign-in as Settings edits them (docs/accounts.md rule 14): what the
// fields show for a stored address, and what is sent back. WinUI-free so the rules are reachable
// from Mailcal.Tests; saving tries the servers first (update_account_endpoints), so a typo never
// costs a working account.

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
    /// The address to store: a host the person wrote a port into as written, as the setup form
    /// dials it; otherwise the bare host when the port is empty or the standard one for
    /// <paramref name="security"/>, and <c>host:port</c> when it is not.
    /// </summary>
    internal static string Joined(
        string host, string port, ConnectionSecurity security, Func<ConnectionSecurity, ushort> standardPort)
    {
        var (bareHost, barePort) = (host.Trim(), port.Trim());
        if (ManualServerField.SplitHost(bareHost).Port.Length > 0)
        {
            return bareHost;
        }
        return barePort.Length == 0 || barePort == Port(standardPort(security))
            ? bareHost
            : $"{bareHost}:{barePort}";
    }

    private static string Port(ushort port) =>
        port.ToString(System.Globalization.CultureInfo.InvariantCulture);
}
