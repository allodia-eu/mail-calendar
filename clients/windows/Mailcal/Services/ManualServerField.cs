// The port and connection security a manual setup form collects for one server, kept out of the
// WinUI view so the rule can be unit-tested without a renderer. The Windows twin of the same
// object on the other three clients (docs/account-autodetect.md).

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// One server's security choice and port on the manual form.
///
/// The port starts at the standard one for the chosen security and follows the picker, until the
/// user types a port of their own; from then on it is theirs and the picker leaves it alone. A
/// server on a port nobody standardised is the whole reason the manual form exists, so the form
/// must never overwrite what was typed for it.
/// </summary>
internal sealed class ManualServerField
{
    private readonly Func<ConnectionSecurity, ushort> _standardPort;
    private bool _typedByHand;

    /// <summary>
    /// The standard port is passed in rather than fetched, for the reason the attendee row's
    /// "Organiser" is: this type is linked into a test assembly that loads no cdylib, so a call
    /// across the FFI here would be a rule no test could reach. <see cref="For"/> is what the
    /// view uses; the numbers themselves are pinned against the core in `mailcal-bindings`.
    /// </summary>
    internal ManualServerField(Func<ConnectionSecurity, ushort> standardPort)
    {
        _standardPort = standardPort;
        Security = ConnectionSecurity.ImplicitTls;
        Port = StandardPort(ConnectionSecurity.ImplicitTls);
    }

    /// <summary>The field for one of an account's two servers, answered by the core.</summary>
    internal static ManualServerField For(MailServerKind kind) =>
        new(security => MailcalBindingsMethods.StandardPort(kind, security));

    /// <summary>How this server's connection is secured. Implicit TLS unless the user says otherwise.</summary>
    internal ConnectionSecurity Security { get; private set; }

    /// <summary>The port as the field shows it. Empty means "whatever the standard one is".</summary>
    internal string Port { get; private set; }

    /// <summary>Whether the picker may still move the port.</summary>
    internal bool FollowsSecurity => !_typedByHand;

    /// <summary>The user picked a security. The port follows only while it is still ours.</summary>
    internal void ChooseSecurity(ConnectionSecurity security)
    {
        Security = security;
        if (!_typedByHand)
        {
            Port = StandardPort(security);
        }
    }

    /// <summary>
    /// The user typed in the port field. Clearing it hands the port back to the picker: an empty
    /// field submits a bare host, which the core resolves to the same standard port, so there is
    /// nothing for a cleared field to mean other than "you choose".
    /// </summary>
    internal void TypePort(string port)
    {
        Port = port;
        _typedByHand = !string.IsNullOrWhiteSpace(port);
        if (!_typedByHand)
        {
            Port = StandardPort(Security);
        }
    }

    /// <summary>
    /// A detected route filled this server in. Its port travels inside the host (`host:port`), so
    /// the field shows what the detection found and stops following the picker.
    /// </summary>
    internal void AdoptDetected(string hostWithOptionalPort, ConnectionSecurity security)
    {
        Security = security;
        var (_, port) = SplitHost(hostWithOptionalPort);
        _typedByHand = port.Length > 0;
        Port = _typedByHand ? port : StandardPort(security);
    }

    /// <summary>The `host:port` this field submits, or the bare host when it carries no port.</summary>
    internal string Dial(string host)
    {
        var typed = host.Trim();
        if (typed.Length == 0 || SplitHost(typed).Port.Length > 0)
        {
            // A host the user already wrote a port into wins: two ports would be a contradiction,
            // and the one in the host field is the one they can see next to the name.
            return typed;
        }
        return Port.Trim().Length == 0 ? typed : $"{typed}:{Port.Trim()}";
    }

    private string StandardPort(ConnectionSecurity security) =>
        _standardPort(security).ToString(System.Globalization.CultureInfo.InvariantCulture);

    /// <summary>
    /// Splits a typed server into host and port. Only an all-digit tail after the last colon is a
    /// port, so an IPv6 literal or a stray colon stays part of the host.
    /// </summary>
    internal static (string Host, string Port) SplitHost(string input)
    {
        var at = input.LastIndexOf(':');
        if (at <= 0 || at == input.Length - 1)
        {
            return (input, string.Empty);
        }
        var tail = input[(at + 1)..];
        foreach (var character in tail)
        {
            if (character is < '0' or > '9')
            {
                return (input, string.Empty);
            }
        }
        return (input[..at], tail);
    }
}
