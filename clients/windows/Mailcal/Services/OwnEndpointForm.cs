// The own AI endpoint form's rules (docs/ai.md, "Where requests go"), WinUI-free and L10n-free so
// Mailcal.Tests can pin them; the words are WritingStyleText. The one that matters most is the key:
// the field is never filled back in, so an empty field has to mean "keep the stored key", or every
// edit of the address would quietly remove it.

using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Why the own endpoint's settings were not saved.</summary>
internal enum OwnEndpointProblem
{
    /// <summary>The address is not a usable URL.</summary>
    Address,

    /// <summary>The address is plain HTTP to somewhere other than this computer.</summary>
    Https,

    /// <summary>No model name was given.</summary>
    Model,

    /// <summary>The secure store refused the key, or the core named no reason.</summary>
    Keystore,
}

/// <summary>The own endpoint form's rules.</summary>
internal static class OwnEndpointForm
{
    /// <summary>
    /// The key the core is given: <c>null</c> for an empty field, which keeps whatever key is
    /// stored (none, when none is), else what was typed.
    /// </summary>
    internal static string? KeyArgument(string typed) => string.IsNullOrWhiteSpace(typed) ? null : typed;

    /// <summary>The problem <paramref name="error"/> reports.</summary>
    internal static OwnEndpointProblem ProblemFor(Exception error) => error switch
    {
        OwnEndpointException.InvalidUrl => OwnEndpointProblem.Address,
        OwnEndpointException.NotHttps => OwnEndpointProblem.Https,
        OwnEndpointException.NoModel => OwnEndpointProblem.Model,
        _ => OwnEndpointProblem.Keystore,
    };

    /// <summary>
    /// The host the consent sheet names as where the words go, or the address as typed when it
    /// does not parse.
    /// </summary>
    internal static string Host(string baseUrl) =>
        Uri.TryCreate(baseUrl.Trim(), UriKind.Absolute, out var uri) && uri.Host.Length > 0
            ? uri.Host
            : baseUrl.Trim();
}
