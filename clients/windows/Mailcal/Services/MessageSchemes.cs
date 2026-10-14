// Which page a message is drawn on (docs/reading-dark-mode.md). WinUI-free on purpose, so the rule
// is pinned in Mailcal.Tests: the reading view applies it to its own ActualTheme and the core's
// preference, and both inputs change while a message is open.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Resolves the scheme a message is rendered in.</summary>
internal static class MessageSchemes
{
    /// <summary>
    /// Dark exactly when the app is painted dark and the reader has left dark messages on.
    /// </summary>
    public static MessageScheme For(bool appDark, bool darkMessages) =>
        appDark && darkMessages ? MessageScheme.Dark : MessageScheme.Light;
}
