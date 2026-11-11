// The Allodia-account half of MailboxModel: the card the core keeps for it, and the changes to its
// sign-in. The core runs the sign-in as its one browser sign-in (MailboxModel.BrowserSignIn.cs opens
// it and hands its redirect back) and the sign-out on a thread of its own; nothing here holds a
// token or a step of either.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // The last of the service's sign-out pages opened, so each is opened once.
    private ulong _allodiaEndSessionOpened;

    /// <summary>Raised on the UI thread when the Allodia account's card may have moved
    /// (<c>Surface.Allodia</c>): Settings → Allodia account and the first-run card read it again,
    /// and nothing else.</summary>
    public event Action? AllodiaChanged;

    /// <summary>The Allodia account's card as the core decided it; null before the core is up.
    /// Local and cheap.</summary>
    // `internal`, like every signature naming a generated UniFFI type: they are emitted internal,
    // so a public one is a CS0051 accessibility error.
    internal AllodiaCard? AllodiaCard() => _app?.AllodiaCard();

    /// <summary>Signs in to an Allodia account in the browser, or creates one with
    /// <paramref name="create"/>. Supersedes a browser sign-in still out.</summary>
    internal void SignInToAllodia(bool create = false)
    {
        Log.Info("allodia: sign-in started");
        _app?.Dispatch(new Intent.Allodia(new AllodiaIntent.SignIn(create, AllodiaOAuthConfig.RedirectUri)));
    }

    /// <summary>Stops <paramref name="browser"/>, the card's sign-in out, which then says
    /// nothing.</summary>
    internal void CancelAllodiaSignIn(BrowserSignIn browser) =>
        _app?.Dispatch(new Intent.Allodia(new AllodiaIntent.CancelSignIn(browser.Attempt)));

    /// <summary>Signs out: the core erases the stored grant and the card names the service's own
    /// page to end its session.</summary>
    /// <remarks>
    /// The erase is local, which is what removing a mail account is too: the grant stays alive at
    /// the service until it expires or the person revokes it there, and the end-session page closes
    /// the browser's session rather than the grant.
    /// </remarks>
    internal void SignOutOfAllodia() => _app?.Dispatch(new Intent.Allodia(new AllodiaIntent.SignOut()));

    /// <summary>
    /// Opens the service's own account page, where someone changes their details or deletes the
    /// account. A page, not a flow: nothing is pending and nothing comes back.
    /// </summary>
    internal void OpenAllodiaAccountPage()
    {
        if (_app?.AllodiaAccountUrl() is { } url)
        {
            _ = Windows.System.Launcher.LaunchUriAsync(new Uri(url));
        }
    }

    private void OnAllodiaSignal()
    {
        if (AllodiaCard() is { } card && AllodiaCardRules.EndSessionToOpen(card, _allodiaEndSessionOpened) is { } end)
        {
            _allodiaEndSessionOpened = end.Id;
            // Best-effort and unreported: this device is signed out whatever the browser does. What
            // it buys is the next sign-in asking who you are rather than completing silently
            // against a session someone thought they had left.
            _ = Windows.System.Launcher.LaunchUriAsync(new Uri(end.Url));
        }
        AllodiaChanged?.Invoke();
    }
}
