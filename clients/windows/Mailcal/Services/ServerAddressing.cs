// How a mail server's address splits into its host and its port, and joins back: the core's rule
// (docs/account-autodetect.md rule 11), so a form never shows one port while the core dials
// another. An IPv6 literal is written in brackets before its port, and bare it names none.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// The core's split and join, passed in rather than called, for the reason the standard port is
/// (<see cref="ManualServerField"/>): the types that use them are linked into a test assembly
/// that loads no cdylib. <see cref="Core"/> is what the app uses; the rule itself is tested in
/// `mailcal-account`.
/// </summary>
internal sealed record ServerAddressing(
    Func<string, (string Host, string Port)> Split,
    Func<string, string, string> Join)
{
    /// <summary>The core's own split and join.</summary>
    internal static ServerAddressing Core { get; } = new(
        address =>
        {
            var parts = MailcalBindingsMethods.ServerAddressParts(address);
            return (parts.Host, parts.Port);
        },
        MailcalBindingsMethods.ServerAddress);
}
