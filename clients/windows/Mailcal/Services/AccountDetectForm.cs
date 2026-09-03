// The pure logic behind the email-first detection flow, factored out of the WinUI view so it can
// be unit-tested without a renderer, the Windows twin of Android's/Apple's DetectedConnectForm
// and the route/prefill helpers. Two things here are load-bearing: how a detection result maps to
// a routed, prefilled form (which tab, which host fields), and that an untrusted result cannot be
// connected until the user approves it (the cross-platform untrusted-settings gate).

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which setup tab a detection result routes to.</summary>
internal enum DetectTab
{
    Imap,
    Jmap,
    Microsoft,
    Google,
}

/// <summary>
/// A routed, prefilled setup form derived from a detection result: whether to drop to the manual
/// tabs, which tab to show, the fields to prefill, whether the settings need explicit approval
/// (untrusted), and, for a manual fallback, why.
/// </summary>
internal sealed record DetectRoute(
    bool IsManual,
    DetectTab Tab,
    string Email,
    string ImapHost,
    string SmtpHost,
    string JmapServer,
    string CaldavUrl,
    bool NeedsApproval,
    MissReason? Reason,
    // How the detected IMAP/SMTP connections are secured, carried to connect so the engine dials
    // implicit TLS or STARTTLS to match. The manual/JMAP/Microsoft routes leave the implicit-TLS
    // default (the manual form offers implicit TLS only today).
    ConnectionSecurity ImapSecurity = ConnectionSecurity.ImplicitTls,
    ConnectionSecurity SmtpSecurity = ConnectionSecurity.ImplicitTls,
    // The issuer the provider's own autoconfig named, when it named one. Carried so the setup
    // form's pre-flight asks that server first rather than probing well-known paths for one the
    // provider has already pointed at (docs/mail-oauth.md rule 4).
    string? OauthIssuer = null);

/// <summary>Pure routing + connect-gating for the detection flow (no WinUI types, so it's testable).</summary>
internal static class AccountDetectForm
{
    /// <summary>Maps a detection result onto a routed, prefilled form.</summary>
    internal static DetectRoute Route(SetupRecommendation recommendation) => recommendation switch
    {
        SetupRecommendation.Jmap jmap => new DetectRoute(
            IsManual: false, Tab: DetectTab.Jmap, Email: jmap.Email,
            ImapHost: string.Empty, SmtpHost: string.Empty, JmapServer: jmap.ServerUrl,
            CaldavUrl: string.Empty, NeedsApproval: !jmap.IsTrusted, Reason: null),

        // A discovered CalDAV endpoint prefills the calendar field (opt-out, the user can clear it);
        // calendar reuses the IMAP credentials at connect.
        SetupRecommendation.Imap imap => new DetectRoute(
            IsManual: false, Tab: DetectTab.Imap, Email: imap.Email,
            ImapHost: imap.ImapHost, SmtpHost: imap.SmtpHost ?? string.Empty, JmapServer: string.Empty,
            CaldavUrl: imap.CaldavUrl ?? string.Empty, NeedsApproval: !imap.IsTrusted, Reason: null,
            ImapSecurity: imap.ImapSecurity, SmtpSecurity: imap.SmtpSecurity,
            OauthIssuer: imap.OauthIssuer),

        SetupRecommendation.Microsoft microsoft => new DetectRoute(
            IsManual: false, Tab: DetectTab.Microsoft, Email: microsoft.Email,
            ImapHost: string.Empty, SmtpHost: string.Empty, JmapServer: string.Empty,
            CaldavUrl: string.Empty, NeedsApproval: false, Reason: null),

        // Like Microsoft, Google is a browser sign-in with nothing to prefill and no untrusted
        // gate; the Early Access confirmation is a separate, view-only gate on the sign-in button.
        SetupRecommendation.Google google => new DetectRoute(
            IsManual: false, Tab: DetectTab.Google, Email: google.Email,
            ImapHost: string.Empty, SmtpHost: string.Empty, JmapServer: string.Empty,
            CaldavUrl: string.Empty, NeedsApproval: false, Reason: null),

        SetupRecommendation.Manual manual => Manual(manual.Reason),

        _ => Manual(MissReason.NothingFound),
    };

    /// <summary>
    /// Whether Connect is allowed for a detected result: the field requirements for the tab, plus
    /// the untrusted-approval gate (when the settings need approval, the user must have approved).
    /// </summary>
    internal static bool CanConnect(DetectTab tab, bool needsApproval, bool approved, string imapHost, string email, string password, string jmapSecret)
    {
        if (needsApproval && !approved)
        {
            return false;
        }
        return tab switch
        {
            DetectTab.Jmap => JmapSetupForm.CanConnect(email, jmapSecret),
            DetectTab.Imap => !string.IsNullOrWhiteSpace(imapHost)
                && !string.IsNullOrWhiteSpace(email)
                && !string.IsNullOrEmpty(password),
            _ => false,
        };
    }

    private static DetectRoute Manual(MissReason reason) => new DetectRoute(
        IsManual: true, Tab: DetectTab.Imap, Email: string.Empty,
        ImapHost: string.Empty, SmtpHost: string.Empty, JmapServer: string.Empty,
        CaldavUrl: string.Empty, NeedsApproval: false, Reason: reason);
}
