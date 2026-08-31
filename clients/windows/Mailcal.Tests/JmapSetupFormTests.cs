// The setup form's JMAP tab, as arithmetic. Two things here are load-bearing and invisible once
// wrong, the same two the Android suite pins (AccountSetupJmapTest.kt):
//   - which fields make Connect tappable: JMAP authenticates with a password OR an API token, never
//     needing both, and never needing a server; and
//   - what the form hands the core: a blank field must arrive as null, since an empty-string server
//     would be taken as a real URL instead of "discover it from my email domain" (docs/jmap.md §4).
//
// This is the WinUI-free half of the form (JmapSetupForm), so it runs in the plain net10.0 test
// assembly, no renderer, no cdylib. It constructs a JmapSetup but never calls into Rust.
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class JmapSetupFormTests
{
    [Fact]
    public void Connect_is_gated_until_an_email_and_the_secret_are_present()
    {
        // Nothing typed, an address alone, or a secret with no address: all un-connectable.
        Assert.False(JmapSetupForm.CanConnect(email: "", secret: ""));
        Assert.False(JmapSetupForm.CanConnect(email: "alice@test.local", secret: ""));
        Assert.False(JmapSetupForm.CanConnect(email: "   ", secret: "secret"));

        // Email + the secret, whichever kind the server issued.
        Assert.True(JmapSetupForm.CanConnect(email: "alice@test.local", secret: "secret"));
        Assert.True(JmapSetupForm.CanConnect(email: "alice@test.local", secret: "tok_123"));
    }

    [Fact]
    public void Build_nulls_the_blank_server()
    {
        // No server typed: it must be null (discover from the domain), not an empty string.
        var setup = JmapSetupForm.Build(email: "alice@test.local", serverUrl: "  ", secret: "secret");

        Assert.Equal("alice@test.local", setup.Email);
        Assert.Null(setup.ServerUrl);
        Assert.Equal("secret", setup.Password);
    }

    [Fact]
    public void Build_sends_an_api_token_as_the_password_so_either_scheme_can_present_it()
    {
        // The collapse's contract: a token typed into the one secret box arrives as `password`,
        // paired with the email as username, presentable as Basic *or* Bearer. Had it gone across
        // as `token` it would be bearer-only, which is the trap that produced the Fastmail report.
        var setup = JmapSetupForm.Build(
            email: "alice@test.local",
            serverUrl: "http://127.0.0.1:18080",
            secret: "tok_123");

        Assert.Equal("alice@test.local", setup.Email);
        Assert.Equal("http://127.0.0.1:18080", setup.ServerUrl);
        Assert.Equal("tok_123", setup.Password);
    }
}
