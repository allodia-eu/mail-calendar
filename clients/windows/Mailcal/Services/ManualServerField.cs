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
    private readonly ServerAddressing _addressing;
    private bool _typedByHand;

    /// <summary>
    /// The standard port and the address's split and join are passed in rather than fetched, for
    /// the reason the attendee row's "Organiser" is: this type is linked into a test assembly
    /// that loads no cdylib, so a call across the FFI here would be a rule no test could reach.
    /// <see cref="For"/> is what the view uses; the numbers themselves are pinned against the
    /// core in `mailcal-bindings`.
    /// </summary>
    internal ManualServerField(Func<ConnectionSecurity, ushort> standardPort, ServerAddressing addressing)
    {
        _standardPort = standardPort;
        _addressing = addressing;
        Security = ConnectionSecurity.ImplicitTls;
        Port = StandardPort(ConnectionSecurity.ImplicitTls);
    }

    /// <summary>The field for one of an account's two servers, answered by the core.</summary>
    internal static ManualServerField For(MailServerKind kind) =>
        new(security => MailcalBindingsMethods.StandardPort(kind, security), ServerAddressing.Core);

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
    /// <para>
    /// Text equal to the port the field already holds is the form's own write coming back, not
    /// typing: WinUI raises TextChanged after the code that set the text has returned, so the view
    /// cannot tell the two apart by when the event arrives, and taking the echo for typing would
    /// stop the port following the picker after its first move.
    /// </para>
    internal void TypePort(string port)
    {
        if (port == Port)
        {
            return;
        }
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
        var (_, port) = _addressing.Split(hostWithOptionalPort);
        _typedByHand = port.Length > 0;
        Port = _typedByHand ? port : StandardPort(security);
    }

    /// <summary>
    /// Back to a fresh field, for a form reopened to add another account: the last account's
    /// detected port must not be shown, and must not be dialled either.
    /// </summary>
    internal void Reset() => AdoptDetected(string.Empty, ConnectionSecurity.ImplicitTls);

    /// <summary>
    /// The `host:port` this field submits, or the bare host when it carries no port, joined as the
    /// core joins it.
    /// </summary>
    internal string Dial(string host)
    {
        var (bare, typed) = _addressing.Split(host);
        // A host the user already wrote a port into wins: two ports would be a contradiction, and
        // the one in the host field is the one they can see next to the name.
        return _addressing.Join(bare, typed.Length > 0 ? typed : Port);
    }

    private string StandardPort(ConnectionSecurity security) =>
        _standardPort(security).ToString(System.Globalization.CultureInfo.InvariantCulture);
}
