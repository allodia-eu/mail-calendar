// The composer's "Draft a reply" rules (docs/ai.md, "Drafting a reply"), WinUI-free so Mailcal.Tests
// can pin them. Each is silent when wrong: the control offered on a forward, a draft asked for in
// the wrong account's style, and, the one that loses work, a draft replacing what the person wrote
// without asking because the editor's answer was misread.

using Allodia.Mailcal.ViewModels;

namespace Allodia.Mailcal.Services;

/// <summary>When the composer offers a drafted reply, and what it asks the core for.</summary>
internal static class DraftReplyGate
{
    /// <summary>
    /// Whether the control is drawn at all: a reply or a reply-all, never a forward or a new
    /// message, and only while AI has somewhere to go.
    /// </summary>
    internal static bool Offered(RichComposeKind kind, bool hasRoute) =>
        hasRoute && kind is RichComposeKind.Reply or RichComposeKind.ReplyAll;

    /// <summary>
    /// The <c>from</c> the core is given: the account the composer sends from when the person
    /// changed it away from <paramref name="answered"/>, the account holding the message, else
    /// <c>null</c>, which drafts in that account's style.
    /// </summary>
    internal static string? From(string? selected, string answered) =>
        selected is null || selected == answered ? null : selected;

    /// <summary>What the person wants the reply to say, or <c>null</c> when they left it empty.</summary>
    internal static string? Intent(string? typed) =>
        string.IsNullOrWhiteSpace(typed) ? null : typed.Trim();

    /// <summary>
    /// Reads <c>composerLeadHasText()</c>'s answer as the WebView returns it, JSON-encoded. Only an
    /// explicit <c>false</c> means nothing would be replaced; anything else, a hook that did not
    /// run included, is treated as text, so the person is asked rather than overwritten.
    /// </summary>
    internal static bool LeadHasText(string? encoded) => encoded?.Trim() != "false";
}
