// The core's split and join for an address with no IPv6 literal in it, standing in for the FFI
// this assembly does not load. The rule itself, brackets included, is the core's and is tested in
// `mailcal-account`; what these suites pin is what a form does with its answer.

using Allodia.Mailcal.Services;

namespace Allodia.Mailcal.Tests;

internal static class PlainAddressing
{
    internal static ServerAddressing Instance { get; } = new(Split, Join);

    private static (string Host, string Port) Split(string address)
    {
        var trimmed = address.Trim();
        var at = trimmed.IndexOf(':');
        if (at < 0)
        {
            return (trimmed, string.Empty);
        }
        var port = trimmed[(at + 1)..];
        return port.All(char.IsAsciiDigit) ? (trimmed[..at], port) : (trimmed, string.Empty);
    }

    private static string Join(string host, string port)
    {
        var (bareHost, barePort) = (host.Trim(), port.Trim());
        return bareHost.Length == 0 || barePort.Length == 0 ? bareHost : $"{bareHost}:{barePort}";
    }
}
