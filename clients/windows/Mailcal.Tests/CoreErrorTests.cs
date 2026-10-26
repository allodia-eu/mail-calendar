// The regression test for a leak that reached the screen: UniFFI's C# codegen builds every error
// variant's exception message as "@v1" + "=" + <message>, so a core failure arrived in the setup
// form as `Couldn't connect: @v1=oauth callback: state mismatch (possible CSRF)`. Nothing in this
// repo writes "@v1", it comes from generated code that is built, never committed, so the only
// thing that can hold the line is a test of the unwrapping itself.
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CoreErrorTests
{
    private static readonly CoreErrorWords Words = new(
        "not accepted",
        "set up already",
        "another account",
        "sign in again",
        "calendar unreachable",
        "contacts unreachable",
        "calendar own sign-in",
        "contacts own sign-in",
        "calendar server changed",
        "contacts server changed",
        error => $"couldn't connect: {error}");

    [Fact]
    public void The_generated_field_wrapper_never_reaches_the_user()
    {
        // Exactly what a failed OAuth callback put on screen before this existed.
        Assert.Equal(
            "oauth callback: state mismatch (possible CSRF)",
            CoreError.Describe("@v1=oauth callback: state mismatch (possible CSRF)", "fallback"));
    }

    [Fact]
    public void A_message_without_the_wrapper_is_left_exactly_as_it_is()
    {
        // A PanicException, a BCL exception, or any future codegen that stops wrapping: unchanged,
        // never "cleaned up" into something the core did not say.
        Assert.Equal("connect: imap: timed out", CoreError.Describe("connect: imap: timed out", "x"));
        Assert.Equal("", CoreError.Describe("", ""));
    }

    [Theory]
    // Only a LEADING wrapper is a wrapper. An "@v1=" later in the text belongs to the core's own
    // message, stripping there would corrupt it, which is worse than the leak being fixed.
    [InlineData("say @v1=hello", "say @v1=hello")]
    // "@v" with no digits, or digits with no "=", is not the wrapper shape.
    [InlineData("@vN=nope", "@vN=nope")]
    [InlineData("@v12nope", "@v12nope")]
    // A multi-digit field index is still the wrapper.
    [InlineData("@v12=deep", "deep")]
    public void Only_the_generated_prefix_shape_is_stripped(string message, string expected) =>
        Assert.Equal(expected, CoreError.Describe(message, "fallback"));

    [Fact]
    public void A_refused_certificate_is_described_by_its_reason_and_never_carries_the_certificate()
    {
        // The generated message lists every field, the certificate's fingerprint, subject and
        // issuer included, and this description is what the app log writes
        // (docs/certificate-exceptions.md rule 9).
        var refused = new uniffi.mailcal_bindings.MailcalException.CertificateRejected(
            "certificate not verified: caldav: transport error",
            new uniffi.mailcal_bindings.RejectedCertificate(
                "localhost", "E0:46:B5:7D", "localhost", null, "Untrusted DAV", null, 1, 2));
        var described = CoreError.Describe(refused);
        Assert.Equal("certificate not verified: caldav: transport error", described);
        Assert.DoesNotContain("E0:46", described);
        Assert.DoesNotContain("Untrusted DAV", described);
    }

    [Fact]
    public void An_empty_core_message_falls_back_rather_than_showing_a_bare_label()
    {
        // "Couldn't connect: " with nothing after it is worse than a type name.
        Assert.Equal("MailcalException", CoreError.Describe("@v1=", "MailcalException"));
        Assert.Equal("MailcalException", CoreError.Describe("@v1=   ", "MailcalException"));
    }

    [Fact]
    public void A_refused_sign_in_is_said_in_words_and_logged_as_the_server_put_it()
    {
        // The server's reply is a status and a body meant for a developer: the person is told in
        // their own language, and the log keeps what the server said.
        var refused = new uniffi.mailcal_bindings.MailcalException.SignInRefused("JMAP HTTP 401: {\"status\":401}");
        Assert.Equal("not accepted", CoreError.Said(refused, Words));
        Assert.Equal("JMAP HTTP 401: {\"status\":401}", CoreError.Describe(refused));
        Assert.Equal(
            "no route to host",
            CoreError.Said(new uniffi.mailcal_bindings.MailcalException.Connect("no route to host"), Words));
    }

    [Fact]
    public void An_account_already_set_up_is_said_in_words_and_logged_without_its_address()
    {
        var twice = new uniffi.mailcal_bindings.MailcalException.AlreadySetUp("alice@example.org@imap.example.org");
        Assert.Equal("set up already", CoreError.Said(twice, Words));
        Assert.Equal("that account is already set up", CoreError.Describe(twice));
    }

    [Fact]
    public void A_sign_in_as_another_account_is_said_in_words()
    {
        var other = new uniffi.mailcal_bindings.MailcalException.DifferentAccount();
        Assert.Equal("another account", CoreError.Said(other, Words));
        Assert.Equal("that sign-in is for a different account", CoreError.Describe(other));
    }

    [Fact]
    public void A_browser_back_from_another_sign_in_is_said_on_its_own_and_logged_plainly()
    {
        var stale = new MailcalException.StaleSignIn();
        Assert.Equal("sign in again", CoreError.Said(stale, Words));
        // Not "couldn't connect": nothing failed to connect, and signing in again settles it.
        Assert.Equal("sign in again", CoreError.SaidAsConnect(stale, Words));
        Assert.Equal("the browser returned from another sign-in", CoreError.Describe(stale));
    }

    [Fact]
    public void Every_other_failure_on_a_connect_route_says_it_did_not_connect()
    {
        Assert.Equal("couldn't connect: another account", CoreError.SaidAsConnect(new MailcalException.DifferentAccount(), Words));
        Assert.Equal("couldn't connect: no route to host", CoreError.SaidAsConnect(new MailcalException.Connect("no route to host"), Words));
    }

    [Theory]
    [InlineData(nameof(AccountCapability.Calendar), nameof(UseProblem.Unreachable), "calendar unreachable")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.Unreachable), "contacts unreachable")]
    [InlineData(nameof(AccountCapability.Calendar), nameof(UseProblem.NeedsOwnSignIn), "calendar own sign-in")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.NeedsOwnSignIn), "contacts own sign-in")]
    [InlineData(nameof(AccountCapability.Calendar), nameof(UseProblem.SignInRefused), "not accepted")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.SignInRefused), "not accepted")]
    [InlineData(nameof(AccountCapability.Calendar), nameof(UseProblem.ServerChanged), "calendar server changed")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.ServerChanged), "contacts server changed")]
    // The core never names mail here; it is read as the calendar.
    [InlineData(nameof(AccountCapability.Mail), nameof(UseProblem.Unreachable), "calendar unreachable")]
    [InlineData(nameof(AccountCapability.Mail), nameof(UseProblem.NeedsOwnSignIn), "calendar own sign-in")]
    public void A_use_that_did_not_connect_says_which_and_why(string capability, string problem, string said) =>
        Assert.Equal(said, CoreError.Said(NotConnected(capability, problem), Words));

    [Fact]
    public void A_use_that_did_not_connect_is_logged_as_a_phrase_rather_than_its_fields()
    {
        var failed = new MailcalException.UseNotConnected(AccountCapability.Contacts, UseProblem.Unreachable);
        Assert.Equal("the address book did not connect: its server could not be reached", CoreError.Describe(failed));
        Assert.Equal(
            "the calendar did not connect: its server needs a sign-in of its own",
            CoreError.Describe(new MailcalException.UseNotConnected(AccountCapability.Calendar, UseProblem.NeedsOwnSignIn)));
        Assert.Equal(
            "the calendar did not connect: it is paused until a server on another site is allowed",
            CoreError.Describe(new MailcalException.UseNotConnected(AccountCapability.Calendar, UseProblem.ServerChanged)));
    }

    [Fact]
    public void A_banner_says_its_own_line_unless_the_remedy_differs()
    {
        Assert.Equal("sign in again", CoreError.SaidOnBanner(new MailcalException.StaleSignIn(), Words, "again failed"));
        Assert.Equal("another account", CoreError.SaidOnBanner(new MailcalException.DifferentAccount(), Words, "again failed"));
        // A refused grant or anything else stays the banner's own line; the cause is in the log.
        Assert.Equal("again failed", CoreError.SaidOnBanner(new MailcalException.SignInRefused("invalid_grant"), Words, "again failed"));
        Assert.Equal("again failed", CoreError.SaidOnBanner(new MailcalException.Connect("token exchange"), Words, "again failed"));
        // A failure the core did not raise, such as a browser that did not open.
        Assert.Equal("again failed", CoreError.SaidOnBanner(null, Words, "again failed"));
    }

    // Named rather than typed: the generated enums are internal, and a public test cannot take them.
    private static MailcalException.UseNotConnected NotConnected(string capability, string problem) =>
        new(Enum.Parse<AccountCapability>(capability), Enum.Parse<UseProblem>(problem));
}
