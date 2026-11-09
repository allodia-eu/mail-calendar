// The separate-server form's sign-in at the server's own provider (docs/mail-oauth.md rule 13):
// whether the form offers it, which attempt an answer belongs to, and what the form does when one
// ends. WinUI-free and L10n-free, so Mailcal.Tests drives every decision; the views draw it.
//
// An attempt is numbered, because the browser hop gives no signal when it is abandoned: a fresh
// press supersedes the attempt still out, the way back retires it, and whatever a superseded or
// retired attempt later reports finds a different number and changes nothing.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Where the form's sign-in stands.</summary>
internal enum SeparateSignInPhase
{
    /// <summary>Nothing out: the button is the offer.</summary>
    Idle,
    /// <summary>The core finds the issuer and registers this app, a blocking call.</summary>
    Starting,
    /// <summary>The browser is open. The form stays usable, and a fresh press supersedes.</summary>
    Waiting,
    /// <summary>The core exchanges the code and connects the new account, a bounded blocking
    /// call with no way back.</summary>
    Completing,
}

/// <summary>How a sign-in ended, as the model reports it: the new account's id, or what to say
/// under the form, or neither for one that was superseded or retired.</summary>
internal sealed record SeparateSignInEnd(string? AccountId, string? Error)
{
    internal static SeparateSignInEnd Added(string accountId) => new(accountId, null);

    internal static SeparateSignInEnd Failed(string error) => new(null, error);

    internal static SeparateSignInEnd Retired { get; } = new(null, null);
}

/// <summary>What the form does once a sign-in has answered.</summary>
/// <param name="Error">Null to go on (setup to its link step, the account's page to a redraw
/// without the form); otherwise what to say under the form, which stays.</param>
internal sealed record SeparateServerNext(string? Error)
{
    internal bool GoesOn => Error is null;
}

/// <summary>Which sign-in the server offers above the password field.</summary>
internal enum SeparateSignInOffer
{
    /// <summary>None: the password alone.</summary>
    None,
    /// <summary>Its own provider's (docs/mail-oauth.md rule 13).</summary>
    Provider,
    /// <summary>Nextcloud's own, where the provider's is not offered (docs/nextcloud-sign-in.md
    /// rule 2).</summary>
    Nextcloud,
}

internal sealed class SeparateServerSignIn
{
    private int _attempt;

    /// <summary>Where the current attempt stands.</summary>
    internal SeparateSignInPhase Phase { get; private set; }

    /// <summary>A Nextcloud grant the form could not make an account of yet, held for the next
    /// press; the way back withdraws it (<see cref="NextcloudGrantHold.Release"/>).</summary>
    internal NextcloudGrantHold Grant { get; } = new();

    /// <summary>What names a form: the account and the uses it moves.</summary>
    internal static string Key(SeparateServerStep step) =>
        $"{step.AccountId}\n{string.Join(',', UnreachedUsesRules.Dav(step.Uses))}";

    /// <summary>Which sign-in the form for <paramref name="step"/> offers: none until the core
    /// draws one.</summary>
    internal static SeparateSignInOffer Offer(SeparateServerStep? step) => step?.Section.SignIn switch
    {
        AccountSeparateSignIn.Provider => SeparateSignInOffer.Provider,
        AccountSeparateSignIn.Nextcloud => SeparateSignInOffer.Nextcloud,
        _ => SeparateSignInOffer.None,
    };

    /// <summary>Whether the form for <paramref name="step"/> offers a sign-in at all.</summary>
    internal static bool Offers(SeparateServerStep? step) => Offer(step) != SeparateSignInOffer.None;

    /// <summary>A press of the button: a new attempt, which supersedes any still out.</summary>
    /// <returns>The attempt's number, which its progress and its end name.</returns>
    internal int Start()
    {
        Phase = SeparateSignInPhase.Starting;
        return ++_attempt;
    }

    /// <summary>The model's progress on <paramref name="attempt"/>.</summary>
    /// <returns>Whether it is the current attempt's; nothing changes otherwise.</returns>
    internal bool Advanced(int attempt, SeparateSignInPhase phase)
    {
        if (attempt != _attempt)
        {
            return false;
        }
        Phase = phase;
        return true;
    }

    /// <summary>The way back (Cancel, Skip, Connect, or the form going away): retires the attempt
    /// out, which then opens no browser and adds no account. The bounded completion is left to
    /// finish.</summary>
    /// <returns>Whether one was retired, so the caller cancels its flow.</returns>
    internal bool Retire()
    {
        if (Phase is SeparateSignInPhase.Idle or SeparateSignInPhase.Completing)
        {
            return false;
        }
        Phase = SeparateSignInPhase.Idle;
        _attempt++;
        return true;
    }

    /// <summary>How <paramref name="attempt"/> ended.</summary>
    /// <returns>What the form does; null when the attempt was superseded or retired, which the
    /// form neither says nor acts on.</returns>
    internal SeparateServerNext? Ended(int attempt, SeparateSignInEnd end)
    {
        if (attempt != _attempt)
        {
            return null;
        }
        Phase = SeparateSignInPhase.Idle;
        return end switch
        {
            { AccountId: not null } => new SeparateServerNext(null),
            { Error: { } error } => new SeparateServerNext(error),
            _ => null,
        };
    }

    /// <summary>Whether the button can be pressed: while nothing is out, and again while the
    /// browser is open, where a press supersedes.</summary>
    internal bool ButtonEnabled => Phase is SeparateSignInPhase.Idle or SeparateSignInPhase.Waiting;

    /// <summary>Whether the rest of the form can be used: everywhere but a blocking core
    /// call.</summary>
    internal bool FormEnabled => ButtonEnabled;

    /// <summary>Whether the attempt's own way back is drawn and live: from the press until the
    /// completion starts.</summary>
    internal bool CancelShown => Phase is SeparateSignInPhase.Starting or SeparateSignInPhase.Waiting;

    /// <summary>Whether the progress ring turns.</summary>
    internal bool Busy => Phase != SeparateSignInPhase.Idle;

    /// <summary>
    /// What a failed sign-in says under the form: <paramref name="failed"/>, since the server's own
    /// reply is meant for a developer, except where the core typed an outcome a person can act
    /// on: a sign-in that came back as another account, a browser that returned from another
    /// sign-in, and a calendar or address book that did not connect. A refused sign-in stays
    /// <paramref name="failed"/>. Callers pass <c>L10n.SetupImapSigninFailed()</c> and
    /// <c>CoreErrorCopy.Words()</c>.
    /// </summary>
    internal static string Said(Exception error, string failed, CoreErrorWords words) => error switch
    {
        MailcalException.DifferentAccount => words.DifferentAccount,
        MailcalException.StaleSignIn => words.StaleSignIn,
        MailcalException.UseNotConnected use => CoreError.NotConnected(use.@capability, use.@problem, words),
        _ => failed,
    };
}
