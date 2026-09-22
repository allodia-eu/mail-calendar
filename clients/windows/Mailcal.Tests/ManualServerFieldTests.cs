// The manual form's port and connection security, as arithmetic: the WinUI-free half of the rule
// in docs/account-autodetect.md. The Windows twin of the same suite on the other three clients.
// What is load-bearing here is that the picker fills the port until the user takes it over, and
// that a port they typed is never overwritten: a server on a non-standard port is the reason the
// manual form exists at all.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ManualServerFieldTests
{
    // The standard ports, as the core answers them. They are written out here because this
    // assembly loads no cdylib; that they are the SAME numbers the core dials is pinned in
    // `mailcal-bindings`'s own suite, which is the only place that can assert it.
    private static ManualServerField Imap() =>
        new(security => security == ConnectionSecurity.ImplicitTls ? (ushort)993 : (ushort)143);

    private static ManualServerField Smtp() =>
        new(security => security == ConnectionSecurity.ImplicitTls ? (ushort)465 : (ushort)587);

    [Fact]
    public void AFreshFieldOffersTheStandardSecurePort()
    {
        Assert.Equal("993", Imap().Port);
        Assert.Equal("465", Smtp().Port);
        Assert.Equal(ConnectionSecurity.ImplicitTls, Imap().Security);
    }

    [Fact]
    public void ThePortFollowsTheSecurityPickerWhileItIsStillOurs()
    {
        var imap = Imap();
        imap.ChooseSecurity(ConnectionSecurity.StartTls);
        Assert.Equal("143", imap.Port);
        imap.ChooseSecurity(ConnectionSecurity.ImplicitTls);
        Assert.Equal("993", imap.Port);

        var smtp = Smtp();
        smtp.ChooseSecurity(ConnectionSecurity.StartTls);
        Assert.Equal("587", smtp.Port);
    }

    [Fact]
    public void APortTypedByHandIsNeverOverwrittenByThePicker()
    {
        var field = Imap();
        field.TypePort("1143");

        field.ChooseSecurity(ConnectionSecurity.StartTls);
        Assert.Equal("1143", field.Port);
        field.ChooseSecurity(ConnectionSecurity.ImplicitTls);
        Assert.Equal("1143", field.Port);
        Assert.False(field.FollowsSecurity);
    }

    [Fact]
    public void ClearingThePortHandsItBackToThePicker()
    {
        var field = Imap();
        field.TypePort("1143");
        field.TypePort("   ");

        Assert.True(field.FollowsSecurity);
        Assert.Equal("993", field.Port);
        field.ChooseSecurity(ConnectionSecurity.StartTls);
        Assert.Equal("143", field.Port);
    }

    [Fact]
    public void TheDialAddressCarriesTheHostAndThePortTogether()
    {
        var field = Imap();
        field.ChooseSecurity(ConnectionSecurity.StartTls);
        Assert.Equal("imap.example.net:143", field.Dial("imap.example.net"));

        field.TypePort("1143");
        Assert.Equal("127.0.0.1:1143", field.Dial("127.0.0.1"));
        Assert.Equal("127.0.0.1:1143", field.Dial("  127.0.0.1  "));
    }

    [Fact]
    public void APortAlreadyTypedIntoTheHostFieldWins()
    {
        // Two ports on one server would be a contradiction, and the one beside the name is the
        // one the user can see.
        var field = Imap();
        field.TypePort("1143");
        Assert.Equal("127.0.0.1:1025", field.Dial("127.0.0.1:1025"));
    }

    [Fact]
    public void AnEmptyHostStaysEmptySoTheConnectGateStillRefusesIt()
    {
        Assert.Equal(string.Empty, Imap().Dial("   "));
    }

    [Fact]
    public void ADetectedRouteBringsItsOwnPortAndStopsFollowingThePicker()
    {
        var field = Imap();
        field.AdoptDetected("imap.example.net:1993", ConnectionSecurity.StartTls);

        Assert.Equal("1993", field.Port);
        Assert.False(field.FollowsSecurity);
        Assert.Equal(ConnectionSecurity.StartTls, field.Security);
    }

    [Fact]
    public void ADetectedRouteWithoutAPortShowsTheStandardOneForWhatWasDetected()
    {
        var field = Imap();
        field.AdoptDetected("imap.example.net", ConnectionSecurity.StartTls);

        Assert.Equal("143", field.Port);
        Assert.True(field.FollowsSecurity);
    }

    // Mirrors the core's own split (mailcal-account's `host_and_addr`), deliberately: the core
    // decides the address actually dialled, so a client that split differently would show one
    // port and connect to another. A bare IPv6 literal is not a server either side accepts.
    [Theory]
    [InlineData("imap.example.net", "imap.example.net", "")]
    [InlineData("imap.example.net:993", "imap.example.net", "993")]
    [InlineData("host:", "host:", "")]
    [InlineData("host:abc", "host:abc", "")]
    [InlineData("::1", ":", "1")]
    public void OnlyAnAllDigitTailIsAPort(string input, string host, string port)
    {
        var split = ManualServerField.SplitHost(input);
        Assert.Equal(host, split.Host);
        Assert.Equal(port, split.Port);
    }
}
