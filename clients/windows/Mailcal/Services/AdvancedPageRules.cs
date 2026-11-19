// What Settings → Advanced draws from the core's AdvancedPage (docs/mcp.md, docs/settings.md): the
// words for the assistant server's status and for what a reset came to, and which signals redraw a
// part of the page rather than move its controls in place. WinUI-free and catalog-free so
// Mailcal.Tests reaches it; the words arrive as parameters.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's words for each <see cref="McpStatus"/>.</summary>
internal sealed record McpStatusWords(
    string Off, string Starting, string Listening, string AnotherInstance, string Unavailable);

/// <summary>The catalog's words for each <see cref="ResetOutcome"/>.</summary>
internal sealed record ResetOutcomeWords(string Done, string Failed);

internal static class AdvancedPageRules
{
    /// <summary>What the line under the assistant switch says.</summary>
    internal static string StatusText(McpStatus status, McpStatusWords words) => status switch
    {
        McpStatus.Off => words.Off,
        McpStatus.Starting => words.Starting,
        McpStatus.Listening => words.Listening,
        McpStatus.AnotherInstance => words.AnotherInstance,
        McpStatus.Unavailable => words.Unavailable,
        _ => words.Unavailable,
    };

    /// <summary>Whether the status line is said at full strength: when the server is on and no
    /// assistant can reach it, which is what the person needs to notice.</summary>
    internal static bool StatusNeedsAttention(McpStatus status) =>
        status is McpStatus.AnotherInstance or McpStatus.Unavailable;

    /// <summary>What is said under the reset button, or null when no reset has ended since the
    /// last one started.</summary>
    internal static string? OutcomeText(ResetOutcome? outcome, ResetOutcomeWords words) => outcome switch
    {
        ResetOutcome.Done => words.Done,
        ResetOutcome.Failed => words.Failed,
        _ => null,
    };

    /// <summary>What the groups under the assistant switch show apart from the positions of their
    /// switches, so a signal that moved only those moves them in place and keeps the focus where
    /// the person left it. Empty while the groups are not drawn.</summary>
    internal static string GrantsFingerprint(McpPanel? panel) => panel is { Enabled: true }
        ? string.Join('\n', panel.Accounts.Select(account => $"{account.AccountId}\t{account.Email}"))
            + "\n\n" + panel.Snippet
        : string.Empty;

    /// <summary>What the reset's button and the line under it show.</summary>
    internal static string ResetFingerprint(AdvancedPage page) =>
        $"{page.Reset.Enabled}\t{page.Reset.Running}\t{page.ResetOutcome?.ToString() ?? "none"}";
}
