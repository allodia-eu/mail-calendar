// Turns an exception thrown across the FFI into something a person can read.
//
// UniFFI's C# codegen builds each error variant's exception message from the variant's positional
// field, literally:
//
//     public Connect(string @v1) : base("@v1" + "=" + @v1) { … }
//
// so `ex.Message` for every error the core raises arrives as `@v1=jmap: …`. Shown straight to the
// user that reads as "Couldn't connect: @v1=oauth callback: state mismatch", a generated field
// name leaking into product copy, which the brand rule (plain, no jargon) does not allow. It is
// invisible in review because nothing in this repo writes "@v1": it comes from generated code that
// is built, never committed, so it only appears when an error path actually runs. (Found while
// verifying the OAuth redirect on Windows: a forged callback surfaced exactly that string.)
//
// The same wrapper reaches the diagnostic log, which is the file a user attaches to a support
// request, so both sides route through here.

using System;

namespace Allodia.Mailcal.Services;

/// <summary>Human-readable text for an exception raised by the Rust core over the FFI.</summary>
internal static class CoreError
{
    /// <summary>
    /// The message to show or log for <paramref name="error"/>, with UniFFI's generated
    /// <c>@vN=</c> field wrapper removed. Safe on any exception: one that carries no wrapper
    /// (a <c>PanicException</c>, or a plain BCL exception) comes back unchanged.
    /// A refused certificate is described by its reason alone, as the core's own text is: the
    /// generated message lists the whole certificate, and this text reaches the app log
    /// (<c>docs/certificate-exceptions.md</c> rule 9).
    /// </summary>
    internal static string Describe(Exception error) => error switch
    {
        uniffi.mailcal_bindings.MailcalException.CertificateRejected refused =>
            Describe(refused.@reason, error.GetType().Name),
        uniffi.mailcal_bindings.MailcalException.SignInRefused refused =>
            Describe(refused.@reason, error.GetType().Name),
        _ => Describe(error.Message, error.GetType().Name),
    };

    /// <summary>
    /// What to show a person for <paramref name="error"/>: <paramref name="signInRefused"/> when
    /// the server refused the sign-in, since its reply is meant for a developer, and
    /// <see cref="Describe(Exception)"/> otherwise. The sentence is passed in because L10n
    /// cannot be linked into the test assembly; callers pass <c>L10n.SigninRefused()</c>.
    /// </summary>
    internal static string Said(Exception error, string signInRefused) =>
        error is uniffi.mailcal_bindings.MailcalException.SignInRefused ? signInRefused : Describe(error);

    /// <summary>
    /// The wrapper-stripping itself, split from the exception so it can be tested directly.
    /// <paramref name="fallback"/> is used when stripping would leave nothing at all to show.
    /// </summary>
    internal static string Describe(string message, string fallback)
    {
        var stripped = StripFieldPrefix(message);
        return string.IsNullOrWhiteSpace(stripped) ? fallback : stripped;
    }

    // Removes a leading "@v<digits>=", and only a leading one. The wrapper is a prefix by
    // construction, so anything later in the text belongs to the core's own message and is left
    // alone rather than guessed at.
    private static string StripFieldPrefix(string message)
    {
        if (!message.StartsWith("@v", StringComparison.Ordinal))
        {
            return message;
        }
        var i = 2;
        while (i < message.Length && char.IsAsciiDigit(message[i]))
        {
            i++;
        }
        // "@v" with no digits, or no "=" after them, is not the wrapper, leave it be.
        return i > 2 && i < message.Length && message[i] == '=' ? message[(i + 1)..] : message;
    }
}
