// Where an account page's sign-in is, which decides what its Sign-in section lets the person press
// (docs/provider-oauth.md rule 13, docs/onboarding.md "Nobody may be stranded on it"). WinUI-free so
// the rule is reachable from Mailcal.Tests.
//
// Closing the browser tab sends nothing back, so a section that disabled its buttons until the
// sign-in ended would be dead until the five-minute cap. The buttons are disabled only while a
// blocking core call of the account's own sign-in runs; while it waits on the person, in the
// browser or on a question, a press supersedes it, and in the browser a Cancel retires it.

namespace Allodia.Mailcal.Services;

/// <summary>What an account's sign-in is doing.</summary>
internal enum SignInStep
{
    /// <summary>A blocking core call: discovery before the browser, the exchange after it, or a
    /// password being tried.</summary>
    Calling,

    /// <summary>Waiting on the person's answer to a question the page asks before the browser.</summary>
    Asking,

    /// <summary>Waiting on the browser, which can hang for as long as the cap allows.</summary>
    Browser,
}

/// <summary>The one sign-in an account page has out, and where it is.</summary>
internal sealed class AccountSignInProgress
{
    private int _attempt;
    private string? _account;
    private SignInStep _step;

    /// <summary>A sign-in or a password switch for <paramref name="accountId"/> has started, with a
    /// blocking call.</summary>
    /// <returns>The attempt, which <see cref="Step"/> and <see cref="End"/> are told about.</returns>
    internal int Start(string accountId)
    {
        _account = accountId;
        _step = SignInStep.Calling;
        return ++_attempt;
    }

    /// <summary>The attempt has moved on. An attempt a newer one replaced changes nothing.</summary>
    internal void Step(int attempt, SignInStep step)
    {
        if (attempt == _attempt && _account is not null)
        {
            _step = step;
        }
    }

    /// <summary>The attempt has ended. An attempt a newer one replaced changes nothing.</summary>
    /// <returns>Whether it was still the latest, whose ending the page says.</returns>
    internal bool End(int attempt)
    {
        if (attempt != _attempt)
        {
            return false;
        }
        _account = null;
        return true;
    }

    /// <summary>Whether a sign-in or a password switch for the account is out.</summary>
    internal bool IsOut(string accountId) => _account == accountId;

    /// <summary>Whether the account's sign-in controls can be pressed: always, except while a
    /// blocking call of its own sign-in runs.</summary>
    internal bool ControlsEnabled(string accountId) => _account != accountId || _step != SignInStep.Calling;

    /// <summary>Whether the section draws a Cancel: while the account's sign-in waits on the
    /// browser. A question asked first has a Cancel of its own.</summary>
    internal bool ShowsCancel(string accountId) => _account == accountId && _step == SignInStep.Browser;
}
