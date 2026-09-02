// The pure decision logic behind a mail account's setup form: what to ask for, once the server
// has answered. Factored out of the WinUI view so it can be unit-tested without a renderer, the
// sibling of JmapSignInGate and answering a harder question than it does.
//
// Three answers rather than two, from docs/mail-oauth.md rule 2, and the middle one is why: a
// provider whose sign-in exists but admits only applications it registered in advance is not the
// same as one that offers none, and one bare password form for both leaves somebody wondering why
// the button their colleague has is missing.
//
// The rules that are load-bearing and invisible once wrong are the same ones the JMAP gate keeps:
// an answer belongs to the account it was asked about, so a slow reply never lights a button for
// a server nobody asked about; and no failure ever leaves somebody with no way in, so a failed
// sign-in gives the password field straight back.

namespace Allodia.Mailcal.Services;

/// <summary>How an IMAP browser sign-in ended.</summary>
internal enum ImapSignInOutcome
{
    /// <summary>The account was connected and stored; the form is done.</summary>
    Added,

    /// <summary>The person backed out (Cancel, or a sign-in that never started).</summary>
    Cancelled,

    /// <summary>Discovery, the browser hop, or the token exchange failed.</summary>
    Failed,
}

/// <summary>What the setup form should ask for, as the mail server answered it.</summary>
internal enum ImapAuthAnswer
{
    /// <summary>Not asked yet, or asked about an account that has since changed.</summary>
    Unknown,

    /// <summary>Sign in with the provider, and a password also works.</summary>
    SignInOrPassword,

    /// <summary>Sign in with the provider, and a password does not work.</summary>
    SignInOnly,

    /// <summary>The provider's sign-in exists but is closed to this application.</summary>
    RegistrationNeeded,

    /// <summary>No sign-in here: the password form, as it always was.</summary>
    Password,
}

/// <summary>
/// What a mail account's setup surface shows: whether to offer the sign-in button, whether the
/// password field belongs on screen at all, and which line of explanation (if any) goes with it.
/// No WinUI types, so it's testable.
/// </summary>
internal sealed class ImapSignInGate
{
    // The fields as last typed (trimmed). Everything below is answered *against these*, so an
    // answer for other fields is inert rather than needing to be actively cleared.
    private string _email = string.Empty;
    private string _server = string.Empty;

    // The key a pre-flight is in flight for, and the key the answer we hold belongs to.
    private string? _askingFor;
    private string? _answeredFor;
    private ImapAuthAnswer _answer = ImapAuthAnswer.Unknown;

    // The key whose sign-in failed, so editing the account takes the note away with it.
    private string? _failedFor;
    private bool _signingIn;

    /// <summary>What the server said, for the account as it now reads.</summary>
    internal ImapAuthAnswer Answer => _answeredFor == Key ? _answer : ImapAuthAnswer.Unknown;

    /// <summary>Whether to show the sign-in button.</summary>
    internal bool ShowButton =>
        Answer is ImapAuthAnswer.SignInOrPassword or ImapAuthAnswer.SignInOnly;

    /// <summary>Whether the button can be pressed (not while a sign-in is already out).</summary>
    internal bool ButtonEnabled => ShowButton && !_signingIn;

    /// <summary>Whether the "signing in didn't work" note is up for the current fields.</summary>
    internal bool ShowFailure => _failedFor == Key;

    /// <summary>
    /// Whether to explain that this provider admits only pre-registered applications. Worth its
    /// own line: without it, this screen is indistinguishable from a provider that has no OAuth,
    /// and somebody who has seen the button elsewhere is left guessing.
    /// </summary>
    internal bool ShowRegistrationNeeded => Answer == ImapAuthAnswer.RegistrationNeeded;

    /// <summary>
    /// Whether the password field belongs on screen.
    /// <para>
    /// Absent in exactly two cases, and both are the field being *wrong* rather than merely
    /// second: while the server is still being asked, because a field that appears and is then
    /// taken away reads as the app changing its mind; and on a server that said it refuses
    /// passwords, where it is a dead end nobody finds until they have typed one.
    /// </para>
    /// <para>
    /// A failed sign-in always brings it back, whatever the server said: it is the route left.
    /// </para>
    /// </summary>
    internal bool ShowPassword => ShowFailure || Answer switch
    {
        ImapAuthAnswer.Unknown => false,
        ImapAuthAnswer.SignInOnly => false,
        _ => true,
    };

    /// <summary>Whether the password field accepts input (not while a sign-in is out).</summary>
    internal bool PasswordEnabled => !_signingIn;

    /// <summary>Records the fields as they now read. Cheap, call it on every keystroke.</summary>
    internal void FieldsChanged(string email, string imapHost)
    {
        _email = email.Trim();
        _server = imapHost.Trim();
    }

    /// <summary>
    /// The key to ask the server about, or <c>null</c> when asking isn't worth a dial: a
    /// half-typed address, no server to dial, an answer we already hold, a question already in
    /// flight for exactly these fields, or a sign-in already running.
    /// </summary>
    internal string? BeginAsking()
    {
        if (_signingIn || _server.Length == 0 || !LooksLikeAddress(_email))
        {
            return null;
        }
        var key = Key;
        if (key == _answeredFor || key == _askingFor)
        {
            return null;
        }
        _askingFor = key;
        return key;
    }

    /// <summary>
    /// Records what the server said about <paramref name="key"/>. An answer superseded by a later
    /// question is dropped, so a slow reply can't overwrite a fresher one.
    /// </summary>
    internal void Answered(string key, ImapAuthAnswer answer)
    {
        if (key != _askingFor)
        {
            return;
        }
        _askingFor = null;
        _answeredFor = key;
        _answer = answer;
    }

    /// <summary>The browser sign-in has started: disable the button and clear any old failure.</summary>
    internal void SignInStarted()
    {
        _signingIn = true;
        _failedFor = null;
    }

    /// <summary>The sign-in finished. Only a genuine failure raises the note; a cancel is not an error.</summary>
    internal void SignInFinished(ImapSignInOutcome outcome)
    {
        _signingIn = false;
        if (outcome == ImapSignInOutcome.Failed)
        {
            _failedFor = Key;
        }
    }

    /// <summary>Back to the initial state, for a form reopened to add another account.</summary>
    internal void Reset()
    {
        _email = string.Empty;
        _server = string.Empty;
        _askingFor = null;
        _answeredFor = null;
        _answer = ImapAuthAnswer.Unknown;
        _failedFor = null;
        _signingIn = false;
    }

    // Case-folded so retyping the same account in different case doesn't re-dial; NUL-joined so
    // no address/server pair can collide with another.
    private string Key => $"{_email.ToLowerInvariant()}\0{_server.ToLowerInvariant()}";

    // Enough of an address to be worth dialling for: the domain is one of the issuer candidates
    // the core probes, so anything less is a wasted round trip.
    private static bool LooksLikeAddress(string email)
    {
        var at = email.IndexOf('@', StringComparison.Ordinal);
        return at > 0 && at < email.Length - 1;
    }
}
