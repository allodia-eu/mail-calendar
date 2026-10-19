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
        // The record carries the account's id, an address; the core's own words carry none.
        uniffi.mailcal_bindings.MailcalException.AlreadySetUp => "that account is already set up",
        uniffi.mailcal_bindings.MailcalException.DifferentAccount => "that sign-in is for a different account",
        uniffi.mailcal_bindings.MailcalException.StaleSignIn => "the browser returned from another sign-in",
        // The generated message is a dump of both fields.
        uniffi.mailcal_bindings.MailcalException.UseNotConnected use =>
            $"{UseName(use.@capability)} did not connect: {ProblemName(use.@problem)}",
        _ => Describe(error.Message, error.GetType().Name),
    };

    /// <summary>
    /// What to show a person for <paramref name="error"/>: the sentence <paramref name="words"/>
    /// holds for each outcome the core types for a person, since the server's reply is meant for
    /// a developer, and <see cref="Describe(Exception)"/> otherwise.
    /// </summary>
    internal static string Said(Exception error, CoreErrorWords words) => error switch
    {
        uniffi.mailcal_bindings.MailcalException.SignInRefused => words.SignInRefused,
        uniffi.mailcal_bindings.MailcalException.AlreadySetUp => words.AlreadySetUp,
        uniffi.mailcal_bindings.MailcalException.DifferentAccount => words.DifferentAccount,
        uniffi.mailcal_bindings.MailcalException.StaleSignIn => words.StaleSignIn,
        uniffi.mailcal_bindings.MailcalException.UseNotConnected use =>
            NotConnected(use.@capability, use.@problem, words),
        _ => Describe(error),
    };

    /// <summary>
    /// <see cref="Said(Exception, CoreErrorWords)"/> on a route that says "Couldn't connect"
    /// around it. A sign-in the browser returned from another request is said on its own: nothing
    /// failed to connect, and its remedy is to sign in again.
    /// </summary>
    internal static string SaidAsConnect(Exception error, CoreErrorWords words) =>
        error is uniffi.mailcal_bindings.MailcalException.StaleSignIn
            ? words.StaleSignIn
            : words.ConnectFailed(Said(error, words));

    /// <summary>
    /// What a banner asking for a sign-in again says after one launched from it failed:
    /// <paramref name="failed"/> ("Signing in again didn't work"), except for the two outcomes
    /// whose remedy differs, a sign-in that came back as another account and a browser that
    /// returned from another sign-in. <paramref name="error"/> is null for a failure the core did
    /// not raise, such as a browser that did not open.
    /// </summary>
    internal static string SaidOnBanner(Exception? error, CoreErrorWords words, string failed) => error switch
    {
        uniffi.mailcal_bindings.MailcalException.StaleSignIn => words.StaleSignIn,
        uniffi.mailcal_bindings.MailcalException.DifferentAccount => words.DifferentAccount,
        _ => failed,
    };

    /// <summary>What a calendar or address book that did not connect on a server of its own
    /// says. <paramref name="capability"/> is never mail here, and is read as the calendar if it
    /// is.</summary>
    internal static string NotConnected(
        uniffi.mailcal_bindings.AccountCapability capability,
        uniffi.mailcal_bindings.UseProblem problem,
        CoreErrorWords words)
    {
        var contacts = capability == uniffi.mailcal_bindings.AccountCapability.Contacts;
        return problem switch
        {
            uniffi.mailcal_bindings.UseProblem.NeedsOwnSignIn => contacts ? words.OwnSignInContacts : words.OwnSignInCalendar,
            uniffi.mailcal_bindings.UseProblem.SignInRefused => words.SignInRefused,
            _ => contacts ? words.UnreachableContacts : words.UnreachableCalendar,
        };
    }

    private static string UseName(uniffi.mailcal_bindings.AccountCapability capability) =>
        capability == uniffi.mailcal_bindings.AccountCapability.Contacts ? "the address book" : "the calendar";

    private static string ProblemName(uniffi.mailcal_bindings.UseProblem problem) => problem switch
    {
        uniffi.mailcal_bindings.UseProblem.NeedsOwnSignIn => "its server needs a sign-in of its own",
        uniffi.mailcal_bindings.UseProblem.SignInRefused => "its server refused the sign-in",
        _ => "its server could not be reached",
    };

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

/// <summary>
/// The sentences <see cref="CoreError.Said(Exception, CoreErrorWords)"/> chooses from. Passed in
/// because L10n cannot be linked into the test assembly; the app builds them from the catalog in
/// <c>CoreErrorCopy</c>.
/// </summary>
/// <param name="ConnectFailed">"Couldn't connect: {error}".</param>
internal sealed record CoreErrorWords(
    string SignInRefused,
    string AlreadySetUp,
    string DifferentAccount,
    string StaleSignIn,
    string UnreachableCalendar,
    string UnreachableContacts,
    string OwnSignInCalendar,
    string OwnSignInContacts,
    Func<string, string> ConnectFailed);
