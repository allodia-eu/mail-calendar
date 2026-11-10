// The separate-server form's sign-in through the browser, at the server's own provider
// (docs/mail-oauth.md rule 13) or with Nextcloud (docs/nextcloud-sign-in.md rule 2): the core's
// browser sign-in, started with AccountPageIntent.SignInSeparateServer and drawn from the section.
// What this host decides is how the section is drawn. WinUI-free and L10n-free, so Mailcal.Tests
// drives every decision; the views word it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which sign-in the server offers above the password field.</summary>
internal enum SeparateSignInOffer
{
    /// <summary>None: the password alone.</summary>
    None,
    /// <summary>Its own provider's (docs/mail-oauth.md rule 13).</summary>
    Provider,
    /// <summary>Nextcloud's own (docs/nextcloud-sign-in.md rule 2).</summary>
    Nextcloud,
}

/// <summary>The sign-in row as the section draws it.</summary>
/// <param name="Offer">Which sign-in the button names; the row is collapsed for none.</param>
/// <param name="Button">The button, exactly the core's control: running while the core starts or
/// completes the sign-in, and pressable again while the browser waits.</param>
/// <param name="Waiting">The sign-in the browser holds, whose Cancel is drawn; null otherwise.</param>
/// <param name="Line">What the line under the button says while it waits; null for the note.</param>
internal sealed record SeparateSignInRow(
    SeparateSignInOffer Offer, ControlState Button, BrowserSignIn? Waiting, SignInWaitLine? Line)
{
    /// <summary>What a redraw compares, so a read that changed nothing keeps the buttons.</summary>
    internal string Key => $"{Offer}/{Button}/{BrowserSignInRules.Fingerprint(Waiting)}";
}

internal static class SeparateServerSignIn
{
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

    /// <summary>The sign-in row for <paramref name="step"/>. The button is the core's control and
    /// nothing else: the core holds Connect while a sign-in is out, and a press then is a fresh
    /// sign-in, which must not wait on it.</summary>
    internal static SeparateSignInRow Row(SeparateServerStep step)
    {
        var section = step.Section;
        var line = BrowserSignInRules.WaitingLine(section.Browser);
        return new SeparateSignInRow(Offer(step), section.SignInControl, line is null ? null : section.Browser, line);
    }

    /// <summary>The account page's own Cancel, which closes the form and with it any sign-in still
    /// out: held while a connect runs and while the core completes a sign-in, neither of which it
    /// would stop.</summary>
    internal static ControlState PageCancel(AccountSeparateServer section) =>
        new(!section.Connect.Running && section.Browser?.Cancel.Enabled != false, false);

    /// <summary>The sign-in a form's going away cancels: one out that can still be cancelled.
    /// Null otherwise, and for one being completed, which is left to finish.</summary>
    internal static BrowserSignIn? ToCancel(AccountSeparateServer? section) =>
        section?.Browser is { Cancel.Enabled: true } browser ? browser : null;
}
