// What Settings → Allodia account and the first-run card draw from the core's AllodiaCard, kept free
// of WinUI and of the catalog so Mailcal.Tests can state it. The core decides every state; this only
// picks which of them a card shows and words what the last sign-in or sign-out came to.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which of its states the Settings card draws.</summary>
internal enum AllodiaCardShows
{
    /// <summary>Sign in and Create an account.</summary>
    SignedOut,
    /// <summary>The browser has the sign-in: progress and Cancel, in place of every button.</summary>
    SigningIn,
    /// <summary>Who is signed in: Manage account, Delete and Sign out.</summary>
    SignedIn,
    /// <summary>The service refused the stored sign-in: Sign in again and Sign out.</summary>
    Ended,
}

/// <summary>The words a failure is said in, from the catalog in the app and plain in a test.</summary>
internal sealed record AllodiaFailureWords(
    string Stale,
    string BrowserFailed,
    string Timeout,
    Func<string, string> Failed,
    Func<AccountChangeRefusal, string> Refused);

internal static class AllodiaCardRules
{
    internal static AllodiaCardShows Shows(AllodiaCard card) => card switch
    {
        { Browser: not null } => AllodiaCardShows.SigningIn,
        { State: AllodiaCardState.SignedIn } => AllodiaCardShows.SignedIn,
        { State: AllodiaCardState.Ended } => AllodiaCardShows.Ended,
        _ => AllodiaCardShows.SignedOut,
    };

    /// <summary>Who the card names, signed in or with the sign-in ended; null when nobody is.</summary>
    internal static AllodiaAccount? Account(AllodiaCard card) => card.State switch
    {
        AllodiaCardState.SignedIn signedIn => signedIn.Account,
        AllodiaCardState.Ended ended => ended.Account,
        _ => null,
    };

    /// <summary>Why the last sign-in or sign-out did not work, the same on both cards; null when
    /// there is nothing to say.</summary>
    internal static string? Said(AllodiaFailure? failure, AllodiaFailureWords words) => failure switch
    {
        AllodiaFailure.SignIn { Refusal: AccountChangeRefusal.StaleSignIn } => words.Stale,
        AllodiaFailure.SignIn { Refusal: AccountChangeRefusal.BrowserNotOpened } => words.BrowserFailed,
        AllodiaFailure.SignIn { Refusal: AccountChangeRefusal.NotFinished } => words.Timeout,
        AllodiaFailure.SignIn signIn => words.Failed(words.Refused(signIn.Refusal)),
        AllodiaFailure.SignOut signOut => words.Failed(signOut.Detail),
        _ => null,
    };

    /// <summary>What a sign-in offered somewhere other than the card says beneath its button: the
    /// last sign-in's failure, and never a sign-out's.</summary>
    internal static string? SignInSaid(AllodiaFailure? failure, AllodiaFailureWords words) =>
        failure is AllodiaFailure.SignIn ? Said(failure, words) : null;

    /// <summary>The first-run card's way back, once the core says the sign-in has run long enough
    /// to need one; null otherwise.</summary>
    internal static ControlState? OnboardingCancel(AllodiaCard card) =>
        card is { Onboarding: AllodiaOnboarding.SigningIn { Escape: true }, Browser: { } browser }
            ? browser.Cancel
            : null;

    /// <summary>The service's sign-out page to open now: one not opened before
    /// (<paramref name="opened"/> is the last id that was).</summary>
    internal static AllodiaEndSession? EndSessionToOpen(AllodiaCard card, ulong opened) =>
        card.EndSession is { } end && end.Id > opened ? end : null;

    /// <summary>What a redraw of the first-run card compares.</summary>
    internal static string OnboardingFingerprint(AllodiaCard? card) => card is null
        ? "-"
        : $"{card.Onboarding}/{Account(card)?.Email}/{card.Failure}/{card.Create.Enabled}|{card.Create.Running}/"
            + $"{(card.Browser is { } browser ? $"{browser.Attempt}:{browser.Cancel.Enabled}|{browser.Cancel.Running}" : "-")}";
}
