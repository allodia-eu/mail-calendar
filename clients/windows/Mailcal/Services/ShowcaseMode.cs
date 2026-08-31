// Screenshot/demo mode: the MAILCAL_SHOWCASE env var brings the app up on the in-memory showcase
// dataset (two fictional accounts, seeded sample content) instead of real accounts, so store
// screenshots need no personal mail. The Store wants a screenshot set per listing language, so
// the var doubles as the language switch: MAILCAL_SHOWCASE=nl gives Dutch chrome *and* Dutch sample
// mail in one launch, with no Settings round-trip and nothing persisted. MAILCAL_SHOWCASE_SCREEN
// then picks which screen the run drives to, so scripts/dev/showcase.sh captures the whole set
// without a single pixel tap.
//
// IsOn is hard-false outside DEBUG, so no showcase path can run in a shipped build, the twin of
// the Apple client's `#if DEBUG` guard in ShowcaseMode.swift and Android's FLAG_DEBUGGABLE check.
// Without it a shipped binary would honour a stray MAILCAL_SHOWCASE in the user's environment and
// silently replace their mailbox with fictional mail.

using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which screen a screenshot run drives to once the mailbox has loaded.</summary>
internal enum ShowcaseScreen
{
    /// <summary>The message list. Windows has a reading pane, so the first message opens into it.</summary>
    List,

    /// <summary>The reply composer for the showcase's designated message, pre-filled with sample text.</summary>
    Reply,

    /// <summary>The settings surface.</summary>
    Settings,

    /// <summary>The settings surface, opened on the Signatures category.</summary>
    Signatures,

    /// <summary>The add-an-account form.</summary>
    AddAccount,

    /// <summary>The calendar grid, opened on today. Screenshot-able on every client.</summary>
    Calendar,

    /// <summary>
    /// The showcase's meeting invitation, opened in the reading pane, the mail-and-calendar
    /// surface: Accept / Maybe / Decline over a preview of the day the meeting would land on.
    /// </summary>
    Invitation,
}

/// <summary>Reads <c>MAILCAL_SHOWCASE</c> and resolves the language the showcase renders in.</summary>
internal static class ShowcaseMode
{
    /// <summary>The trimmed, lower-cased env var; null when unset.</summary>
    private static string? Raw =>
        Environment.GetEnvironmentVariable("MAILCAL_SHOWCASE")?.Trim().ToLowerInvariant();

    /// <summary>
    /// Whether screenshot mode is on, <c>MAILCAL_SHOWCASE</c> set to anything other than
    /// <c>0</c>/<c>false</c>/<c>no</c>/<c>off</c>. Always <c>false</c> in a release build.
    /// </summary>
    public static bool IsOn
    {
        get
        {
#if DEBUG
            return Raw is not (null or "" or "0" or "false" or "no" or "off");
#else
            return false;
#endif
        }
    }

    /// <summary>
    /// The language the var pins the whole app to (any locale the catalog ships, e.g. "de"), or
    /// null when it names no language (<c>MAILCAL_SHOWCASE=1</c>) and the app's own language choice
    /// stands. Applied for the session only, so a screenshot run never rewrites the developer's
    /// stored preference. Gated on <see cref="IsOn"/>, so a release build can't have its chrome
    /// language flipped by a stray env var either.
    /// </summary>
    public static string? LanguageOverride =>
        IsOn && Raw is not null && L10n.Locales.Contains(Raw) ? Raw : null;

    /// <summary>
    /// The screen to drive to; <see cref="ShowcaseScreen.List"/> when
    /// <c>MAILCAL_SHOWCASE_SCREEN</c> is unset or names no screen we know.
    /// </summary>
    public static ShowcaseScreen Screen => ParseScreen(
        Environment.GetEnvironmentVariable("MAILCAL_SHOWCASE_SCREEN"));

    /// <summary>
    /// Maps a <c>MAILCAL_SHOWCASE_SCREEN</c> value onto a screen, falling back to
    /// <see cref="ShowcaseScreen.List"/>. The flag spellings are the cross-client contract
    /// (scripts/dev/showcase.sh passes the same names to every client), matched literally rather
    /// than by Enum.TryParse, "addaccount" is not a spelling we accept.
    /// </summary>
    public static ShowcaseScreen ParseScreen(string? raw) => raw?.Trim().ToLowerInvariant() switch
    {
        "reply" => ShowcaseScreen.Reply,
        "settings" => ShowcaseScreen.Settings,
        "signatures" => ShowcaseScreen.Signatures,
        "add-account" => ShowcaseScreen.AddAccount,
        "calendar" => ShowcaseScreen.Calendar,
        "invitation" => ShowcaseScreen.Invitation,
        _ => ShowcaseScreen.List,
    };

    /// <summary>
    /// The locale to seed the showcase mailbox and calendar in: the pinned language when the var
    /// names one, else whatever the chrome resolves to, the stored choice, falling back to the
    /// OS language. Dutch chrome over English mail reads as a broken screenshot, so the sample
    /// content always follows the UI. The code → locale mapping lives in the core, so all three
    /// clients seed the same content for the same language.
    /// </summary>
    public static ShowcaseLocale SeedLocale =>
        MailcalBindingsMethods.ShowcaseLocaleForLanguage(LanguageOverride ?? StoredOrSystemLanguage());

    /// <summary>
    /// The sample reply text to open the composer pre-filled with when replying to the showcase's
    /// designated message, else <c>null</c> (every other message, and every non-showcase run, keeps
    /// the normal empty composer). Lets the "replying to a mail" store screenshot show a written
    /// reply, in the language the chrome renders in. Plain text, see composer Gate 11.
    /// </summary>
    public static string? ReplyText(string account, string key)
    {
        if (!IsOn)
        {
            return null;
        }
        var reply = MailcalBindingsMethods.ShowcaseReply(SeedLocale);
        return account == reply.Account && key == reply.MessageKey ? reply.Text : null;
    }

    /// <summary>The app's effective language: its stored choice, else the OS's.</summary>
    private static string StoredOrSystemLanguage()
    {
        var stored = LanguageStore.Read();
        return L10n.Locales.Contains(stored)
            ? stored
            : CultureInfo.CurrentUICulture.TwoLetterISOLanguageName;
    }
}
