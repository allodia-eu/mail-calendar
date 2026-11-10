// An account's browser sign-in, which the core holds from the press to the account it changes
// (docs/waiting.md, docs/provider-oauth.md rule 13): what this host still decides about it. Where
// the browser returns for the account's kind, when the browser is opened, which redirect is the
// sign-in's rather than a setup flow's, and what its Sign-in section says while it waits.
// WinUI-free and L10n-free, so each rule is reachable from Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Where a sign-in started for an account of a kind asks the browser to return.</summary>
internal enum SignInRedirect
{
    /// <summary>The Microsoft sign-in's custom-scheme redirect.</summary>
    Microsoft,
    /// <summary>A loopback port bound for this sign-in alone.</summary>
    Google,
    /// <summary>A standards account's provider, under the app's scheme.</summary>
    Imap,
    /// <summary>A JMAP account's provider, under the app's scheme.</summary>
    Jmap,
    /// <summary>Nothing returns: a calendar and contacts account signs in with Nextcloud, whose
    /// server the core asks itself.</summary>
    None,
}

/// <summary>What the Sign-in section says while the browser holds a sign-in.</summary>
internal enum SignInWaitLine
{
    /// <summary>The browser will return to the app.</summary>
    Redirect,
    /// <summary>Nothing returns; the app notices when the person has finished.</summary>
    Nextcloud,
}

internal static class BrowserSignInRules
{
    /// <summary>The redirect a sign-in for an account of <paramref name="kind"/> is started
    /// with.</summary>
    internal static SignInRedirect RedirectOf(AccountKind kind) => kind switch
    {
        AccountKind.Microsoft => SignInRedirect.Microsoft,
        AccountKind.Google => SignInRedirect.Google,
        AccountKind.Imap => SignInRedirect.Imap,
        AccountKind.Jmap => SignInRedirect.Jmap,
        _ => SignInRedirect.None,
    };

    /// <summary>The browser step to open now: the sign-in waits on the browser, and it has not
    /// been opened for this attempt yet (<paramref name="opened"/>). Null otherwise.</summary>
    internal static BrowserSignInStage.InBrowser? ToOpen(BrowserSignIn? record, ulong? opened) =>
        record is { Stage: BrowserSignInStage.InBrowser browser } && record.Attempt != opened ? browser : null;

    /// <summary>Whether <paramref name="incoming"/>, a redirect the OS handed the app, is the
    /// sign-in's: it waits on the browser at a redirect of the same scheme and host. Any other
    /// belongs to a setup flow.</summary>
    internal static bool Returns(BrowserSignIn? record, Uri incoming) =>
        record is { Stage: BrowserSignInStage.InBrowser { RedirectUri: { } redirect } }
        && Uri.TryCreate(redirect, UriKind.Absolute, out var expected)
        && string.Equals(expected.Scheme, incoming.Scheme, StringComparison.OrdinalIgnoreCase)
        && string.Equals(expected.Host, incoming.Host, StringComparison.OrdinalIgnoreCase);

    /// <summary>
    /// Whether the loopback at <paramref name="redirect"/>, bound when the sign-in was started,
    /// is still what the sign-in returns to: while it starts or asks, and while it waits on the
    /// browser at that address. Once it has been listened on for an attempt
    /// (<paramref name="listened"/>), only that attempt keeps it.
    /// </summary>
    internal static bool KeepsLoopback(BrowserSignIn? record, string redirect, ulong? listened) => record switch
    {
        null => false,
        _ when listened is { } attempt && record.Attempt != attempt => false,
        { Stage: BrowserSignInStage.Starting or BrowserSignInStage.Asking } => listened is null,
        { Stage: BrowserSignInStage.InBrowser browser } => browser.RedirectUri == redirect,
        _ => false,
    };

    /// <summary>The line the Sign-in section draws with its Cancel while the browser holds the
    /// sign-in; null at every other step.</summary>
    internal static SignInWaitLine? WaitingLine(BrowserSignIn? browser) => browser?.Stage switch
    {
        BrowserSignInStage.InBrowser { RedirectUri: null } => SignInWaitLine.Nextcloud,
        BrowserSignInStage.InBrowser => SignInWaitLine.Redirect,
        _ => null,
    };

    /// <summary>The question the sign-in stands at before the browser opens, with what it would
    /// cost; null at every other step.</summary>
    internal static IReadOnlyList<AccountCapability>? Asking(BrowserSignIn? browser) =>
        browser?.Stage is BrowserSignInStage.Asking asking ? UnreachedUsesRules.Dav(asking.Unreached) : null;

    /// <summary>What a redraw compares: the attempt, its step and its Cancel.</summary>
    internal static string Fingerprint(BrowserSignIn? browser) => browser is null
        ? "-"
        : $"{browser.Attempt}/{browser.Stage.GetType().Name}/{browser.Cancel.Enabled}|{browser.Cancel.Running}";
}
