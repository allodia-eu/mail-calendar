// The Google (Gmail + Google Calendar) sign-in half of MailboxModel, split out to keep each file
// under the 500-line limit and to sit beside the Microsoft flow in MailboxModel.Accounts.cs. It
// mirrors SignInWithMicrosoft exactly, with one platform difference: Windows uses Google's
// recommended Desktop flow, an http://127.0.0.1 loopback HttpListener (see GoogleOAuth.cs),
// instead of a custom-scheme protocol activation. A public installed-app client secured by PKCE
// (owned by the core), which for a Google Desktop client also carries a non-confidential
// client_secret Google's token endpoint requires, injected into the core at build time, along
// with the client id (BUILDING.md). State stays in Rust; this
// file owns only opening the browser and catching the loopback redirect. The Early Access gate (a
// confirmed sign-up) is the setup view's job, before this ever runs.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Starts the Google sign-in: picks a free loopback port, opens the authorization URL in the
    /// user's browser (which reuses its logged-in Google session) and captures the redirect on a
    /// one-shot HttpListener, then completes the flow (code exchange + connect) off the UI thread
    /// and stores the config. The account appears at once; its first sync runs in the background. A
    /// cancel/failure keeps the form up with <see cref="SetupError"/>. The Windows twin of macOS's
    /// signInWithGoogle.
    /// </summary>
    /// <remarks>
    /// Deliberately NOT guarded on <see cref="IsSubmitting"/>: a request that arrives while one is
    /// outstanding supersedes it (see <see cref="SignInFlight"/>), because a sign-in abandoned in
    /// the browser is indistinguishable from one still in progress. The setup form's own buttons
    /// are already disabled while submitting, so the removed guard cost no double-fire protection
    /// there, its only remaining effect was to make the reconnect banner's button dead.
    /// </remarks>
    public void SignInWithGoogle(string? loginHint = null)
    {
        if (_connecting)
        {
            return;
        }
        if (_app is null)
        {
            SetupError = "Could not open the app. Please relaunch.";
            return;
        }
        _ = _signIn.RunAsync(cancel => SignInWithGoogleAsync(loginHint, cancel));
    }

    /// <summary>
    /// Aborts a Google sign-in that's waiting on the loopback redirect (the user pressed Cancel).
    /// Safe to call when none is in flight. The awaiting flow unwinds cleanly and re-enables the form.
    /// </summary>
    public void CancelGoogleSignIn() => _signIn.Cancel();

    private async Task SignInWithGoogleAsync(string? loginHint, CancellationToken cancelToken)
    {
        // IsSigningIn drives the (enabled) Cancel; IsSubmitting keeps the setup form's own buttons
        // from firing twice (they bind to it) while the browser step is outstanding.
        IsSigningIn = true;
        IsSubmitting = true;
        SetupError = null;
        try
        {
            // Pick the free loopback port BEFORE building the request: begin_google_login must be
            // handed the exact redirect URI the browser will return to.
            using var loopback = new GoogleLoopback();
            var start = MailcalBindingsMethods.BeginGoogleLogin(
                loopback.RedirectUri,
                // The address the user is connecting (from autodetection), so Google targets that
                // account instead of another already signed in in the browser; null/blank ⇒ the picker.
                string.IsNullOrWhiteSpace(loginHint) ? null : loginHint);
            // Open the default browser (where the user is usually already signed in to Google).
            await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
            var callbackUrl = await WaitForGoogleCallbackAsync(loopback, cancelToken);
            // The code exchange + folder connect block, so run them off the UI thread.
            var row = await Task.Run(() => _app!.CompleteGoogleLogin(start.Pending, callbackUrl));
            SetupError = null;
            NeedsSetup = false;
            AddingAccount = false;
            Log.Info($"google account added: {row.Email}");
            // This route never touches AddAccountAsync, so the pass is owed here: without it the
            // account stays on this device until the next launch, and its card in Settings draws
            // no sharing control at all (docs/settings.md, category 9).
            SyncAfterAccountChange();
            // Land directly in the newly connected account rather than the unified inbox, so the
            // user sees their mail arriving where they just signed in (the core owns selection; this
            // dispatches the select intent and the snapshot expands the account in the sidebar).
            SelectAccount(row.Id);
        }
        catch (OperationCanceledException)
        {
            // The user pressed Cancel while the browser step was outstanding, return to the form
            // quietly (no error banner); the stopped listener ignores any late redirect.
            Log.Info("google sign-in cancelled by user");
        }
        catch (Exception ex)
        {
            Log.Error($"google sign-in failed: {CoreError.Describe(ex)}");
            SetupError = L10n.StatusConnectFailed(CoreError.Describe(ex));
        }
        finally
        {
            IsSigningIn = false;
            IsSubmitting = false;
        }
    }

    // Awaits the browser's loopback redirect, bounding the otherwise unbounded wait on two outcomes
    // besides success: the user cancels (<paramref name="cancel"/>), or a generous cap elapses. Both
    // stop the loopback listener; a user cancel re-throws as OperationCanceledException, a lapsed cap
    // as a TimeoutException. Mirrors ProtocolAuthCallback.Registration.WaitAsync, which does the
    // same for the custom-scheme flows (Microsoft, JMAP).
    private static async Task<string> WaitForGoogleCallbackAsync(GoogleLoopback loopback, CancellationToken cancel)
    {
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancel);
        deadline.CancelAfter(TimeSpan.FromMinutes(5));
        try
        {
            return await loopback.WaitForCallbackAsync(deadline.Token);
        }
        catch (OperationCanceledException)
        {
            cancel.ThrowIfCancellationRequested(); // user pressed Cancel -> OperationCanceledException
            throw new TimeoutException("Google sign-in timed out. Please try again.");
        }
    }
}
