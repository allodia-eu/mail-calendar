// Which store a debug launch boots, read from MAILCAL_DEV_ACCOUNT. The demo account is the core's
// in-memory store and nothing else: a test run against it that reached the stored accounts would
// be acting on the developer's own mail, so the choice is made here, before any account is read,
// and pinned in Mailcal.Tests. WinUI-free for that reason.

using System;

namespace Allodia.Mailcal.Services;

internal enum DevBoot
{
    /// <summary>The accounts the credential store holds, or a harness's beside them.</summary>
    Accounts,

    /// <summary>The core's in-memory demo account (<c>MAILCAL_DEV_ACCOUNT=demo</c>).</summary>
    Demo,
}

internal static class DevBootRules
{
    /// <summary>What <paramref name="raw"/>, the value of <c>MAILCAL_DEV_ACCOUNT</c>, boots.
    /// Trimmed and case ignored, as the harness modes are.</summary>
    internal static DevBoot Resolve(string? raw) =>
        string.Equals(raw?.Trim(), "demo", StringComparison.OrdinalIgnoreCase) ? DevBoot.Demo : DevBoot.Accounts;
}
