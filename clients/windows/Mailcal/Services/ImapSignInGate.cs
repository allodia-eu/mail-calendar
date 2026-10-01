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
// sign-in gives the password field straight back. Two more are rule 8's: a deadline races the
// question, and a password field the form has shown stays shown while a later question is asked.

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
    /// <summary>
    /// How long the form waits for the server before it asks for a password (docs/mail-oauth.md
    /// rule 8). The core bounds its own work well inside this, so it is reached only by a core
    /// that has stopped answering.
    /// </summary>
    internal static readonly TimeSpan Deadline = TimeSpan.FromSeconds(10);

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

    // Whether the password field is on screen and stays there while the account is asked about
    // again. Set by every answer that draws it and by the person choosing it; cleared only by an
    // answer that a password does not work.
    private bool _passwordHeld;

    /// <summary>What the server said, for the account as it now reads.</summary>
    internal ImapAuthAnswer Answer => _answeredFor == Key ? _answer : ImapAuthAnswer.Unknown;

    /// <summary>Whether to show the sign-in button.</summary>
    internal bool ShowButton =>
        Answer is ImapAuthAnswer.SignInOrPassword or ImapAuthAnswer.SignInOnly;

    /// <summary>Whether the button can be pressed (not while a sign-in is already out).</summary>
    internal bool ButtonEnabled => ShowButton && !_signingIn;

    /// <summary>
    /// Whether the sign-in is the form's primary action: offered, and no password field beside it.
    /// Once the field is on screen Connect leads and the sign-in stays as an ordinary button.
    /// </summary>
    internal bool SignInLeads => ShowButton && !ShowPassword;

    /// <summary>
    /// Whether to offer "Use a password instead": the server takes a password as well as a
    /// sign-in, and the field is not on screen yet (rule 2: behind means not drawn until asked
    /// for).
    /// </summary>
    internal bool ShowPasswordInstead => Answer == ImapAuthAnswer.SignInOrPassword && !ShowPassword;

    /// <summary>Whether the server is being asked about the account as it now reads.</summary>
    internal bool Checking => _askingFor is not null && _askingFor == Key;

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
    /// Not before the first answer, because a field that appears and is then taken away reads as
    /// the app changing its mind; not on a server that said it refuses passwords, where it is a
    /// dead end nobody finds until they have typed one; and not beside a sign-in until the person
    /// asks for it.
    /// </para>
    /// <para>
    /// Once shown it stays while an edited account is asked about again, for the same reason it
    /// waits for the first answer: only an answer that a password does not work takes it away. A
    /// failed sign-in always brings it back, whatever the server said: it is the route left.
    /// </para>
    /// </summary>
    internal bool ShowPassword => ShowFailure || Answer switch
    {
        ImapAuthAnswer.SignInOnly => false,
        ImapAuthAnswer.Password or ImapAuthAnswer.RegistrationNeeded => true,
        _ => _passwordHeld,
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
        _passwordHeld = answer switch
        {
            ImapAuthAnswer.Password or ImapAuthAnswer.RegistrationNeeded => true,
            ImapAuthAnswer.SignInOnly => false,
            _ => _passwordHeld,
        };
    }

    /// <summary>
    /// The deadline ran out on the question about <paramref name="key"/>. If that question is
    /// still the one being asked it is answered with a password, rule 7's answer to every question
    /// that went unanswered, and the core's reply, whenever it comes, is dropped: only the first
    /// answer for an account counts, because a late one would rebuild a form somebody is typing
    /// into. Returns whether this decided anything.
    /// </summary>
    internal bool DeadlinePassed(string key)
    {
        if (key != _askingFor)
        {
            return false;
        }
        Answered(key, ImapAuthAnswer.Password);
        return true;
    }

    /// <summary>The person chose "Use a password instead": the field is theirs from now on.</summary>
    internal void RevealPassword() => _passwordHeld = true;

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
            _passwordHeld = true;
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
        _passwordHeld = false;
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
