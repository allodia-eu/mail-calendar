// The regression test for a leak that reached the screen: UniFFI's C# codegen builds every error
// variant's exception message as "@v1" + "=" + <message>, so a core failure arrived in the setup
// form as `Couldn't connect: @v1=oauth callback: state mismatch (possible CSRF)`. Nothing in this
// repo writes "@v1", it comes from generated code that is built, never committed, so the only
// thing that can hold the line is a test of the unwrapping itself.
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CoreErrorTests
{
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
    public void An_empty_core_message_falls_back_rather_than_showing_a_bare_label()
    {
        // "Couldn't connect: " with nothing after it is worse than a type name.
        Assert.Equal("MailcalException", CoreError.Describe("@v1=", "MailcalException"));
        Assert.Equal("MailcalException", CoreError.Describe("@v1=   ", "MailcalException"));
    }
}
