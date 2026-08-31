// The pure decision logic behind the setup form's JMAP "Sign in with your provider" button,
// factored out of the WinUI view so it can be unit-tested without a renderer, the sibling of
// JmapSetupForm / AccountDetectForm. Three things here are load-bearing and invisible once wrong:
//
//   - the button is offered ONLY for a server the core's pre-flight said actually offers
//     discoverable OAuth (crates/mailcal-bindings/src/jmap_oauth.rs). Sign-in is discoverable, not
//     guaranteed: a server may publish no metadata or no open registration, and a button that
//     dead-ends is worse than no button;
//   - an answer belongs to the address/server it was asked about. The probe is a slow network
//     call, so by the time it returns the user may have typed on, a stale "yes" must never light
//     the button for a server nobody asked about;
//   - a failed sign-in leaves the password/API-token field usable. OAuth is an ADDITION to that
//     field, never a replacement, so no failure may ever leave the user with no way in.

namespace Allodia.Mailcal.Services;

/// <summary>How a JMAP browser sign-in ended.</summary>
internal enum JmapSignInOutcome
{
    /// <summary>The account was connected and stored; the form is done.</summary>
    Added,

    /// <summary>The user backed out (Cancel, or a sign-in that never started).</summary>
    Cancelled,

    /// <summary>Discovery, the browser hop, or the token exchange failed.</summary>
    Failed,
}

/// <summary>
/// What the JMAP tab shows for the "Sign in with your provider" flow: whether to offer the button
/// at all, whether it can be pressed, whether the failure note is up, and whether the manual
/// secret field stays usable. No WinUI types, so it's testable.
/// </summary>
internal sealed class JmapSignInGate
{
    // The fields as last typed (trimmed). Everything below is answered *against these*, so an
    // answer for other fields is inert rather than needing to be actively cleared.
    private string _email = string.Empty;
    private string _server = string.Empty;

    // The key a probe is in flight for, and the key the answer we hold belongs to.
    private string? _probingFor;
    private string? _answeredFor;
    private bool _available;

    // The key whose sign-in failed, so editing the address takes the note away with it.
    private string? _failedFor;
    private bool _signingIn;

    // Whether these fields are being shown as a DETECTED result rather than the manual form. It
    // changes one thing only: whether the manual secret is shown BESIDE an offered sign-in.
    private bool _detectedCard;

    /// <summary>Whether to show the sign-in button: this server said it offers sign-in.</summary>
    internal bool ShowButton => _available && _answeredFor == Key;

    /// <summary>Whether the button can be pressed (not while a sign-in is already out).</summary>
    internal bool ButtonEnabled => ShowButton && !_signingIn;

    /// <summary>Whether the "signing in didn't work" note is up for the current fields.</summary>
    internal bool ShowFailure => _failedFor == Key;

    /// <summary>
    /// Whether the password/API-token field stays usable. Only an in-flight sign-in takes it away
    /// (it would be submitting into a flow already running); a failure gives it straight back.
    /// </summary>
    internal bool ManualEnabled => !_signingIn;

    /// <summary>
    /// Whether to show the manual way in at all, the secret field, the server, and the Connect
    /// that submits them.
    /// <para>
    /// On a <b>detected</b> card whose server offers sign-in, it is hidden: showing both asks the
    /// user to choose between two routes to the same place when the better one is a single click,
    /// and the copy contradicts itself ("just add your password" beside "there's no API token to
    /// create"). Hidden is not gone, a sign-in that <b>fails</b> brings it straight back, and the
    /// manual form always shows it, so no server that declines this flow can leave a user stuck.
    /// </para>
    /// <para>
    /// The manual form is deliberately unaffected: someone who chose to set the account up by hand
    /// asked for the fields. Mirrors Android, where the offer sits above the secret on the manual
    /// screen but replaces it on the detected card (<c>AccountSetupDetect.kt</c>).
    /// </para>
    /// </summary>
    internal bool ShowManualSecret => !_detectedCard || !ShowButton || ShowFailure;

    /// <summary>
    /// Records whether these fields are a detected result (<c>true</c>) or the manual form
    /// (<c>false</c>), the only thing <see cref="ShowManualSecret"/> reads beyond the sign-in
    /// state itself.
    /// </summary>
    internal void CardChanged(bool detected) => _detectedCard = detected;

    /// <summary>Records the fields as they now read. Cheap, call it on every keystroke.</summary>
    internal void FieldsChanged(string email, string serverUrl)
    {
        _email = email.Trim();
        _server = serverUrl.Trim();
    }

    /// <summary>
    /// The key to probe availability for, or <c>null</c> when a probe isn't worth a round trip:
    /// a half-typed address, an answer we already hold, a probe already in flight for exactly
    /// these fields, or a sign-in already running.
    /// </summary>
    internal string? BeginProbe()
    {
        if (_signingIn || !LooksLikeAddress(_email))
        {
            return null;
        }
        var key = Key;
        if (key == _answeredFor || key == _probingFor)
        {
            return null;
        }
        _probingFor = key;
        return key;
    }

    /// <summary>
    /// Records what the probe for <paramref name="key"/> answered. An answer superseded by a
    /// later probe is dropped, so a slow reply can't overwrite a fresher one.
    /// </summary>
    internal void Probed(string key, bool available)
    {
        if (key != _probingFor)
        {
            return;
        }
        _probingFor = null;
        _answeredFor = key;
        _available = available;
    }

    /// <summary>The browser sign-in has started: disable the button and clear any old failure.</summary>
    internal void SignInStarted()
    {
        _signingIn = true;
        _failedFor = null;
    }

    /// <summary>The sign-in finished. Only a genuine failure raises the note, a cancel is not an error.</summary>
    internal void SignInFinished(JmapSignInOutcome outcome)
    {
        _signingIn = false;
        if (outcome == JmapSignInOutcome.Failed)
        {
            _failedFor = Key;
        }
    }

    /// <summary>Back to the initial state, for a form reopened to add another account.</summary>
    internal void Reset()
    {
        _email = string.Empty;
        _server = string.Empty;
        _probingFor = null;
        _answeredFor = null;
        _available = false;
        _failedFor = null;
        _signingIn = false;
        _detectedCard = false;
    }

    // Case-folded so retyping the same address in different case doesn't re-probe; NUL-joined so
    // no address/server pair can collide with another.
    private string Key => $"{_email.ToLowerInvariant()}\0{_server.ToLowerInvariant()}";

    // Enough of an address to have a domain to discover against, the probe derives the server
    // from the domain, so anything less is a guaranteed miss (and a wasted round trip).
    private static bool LooksLikeAddress(string email)
    {
        var at = email.IndexOf('@', StringComparison.Ordinal);
        return at > 0 && at < email.Length - 1;
    }
}
