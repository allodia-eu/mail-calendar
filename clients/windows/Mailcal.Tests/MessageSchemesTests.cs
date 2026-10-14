// Which page a message is drawn on (docs/reading-dark-mode.md). The reading view feeds this its
// own ActualTheme and the core's preference; what it must never do is darken a message in a light
// app, which no screenshot of a light app would show was wrong.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class MessageSchemesTests
{
    [Fact]
    public void AMessageIsDarkInADarkAppWithDarkMessagesOn() =>
        Assert.Equal(MessageScheme.Dark, MessageSchemes.For(appDark: true, darkMessages: true));

    [Fact]
    public void TheSunSendsADarkAppsMessagesToTheirLightPage() =>
        Assert.Equal(MessageScheme.Light, MessageSchemes.For(appDark: true, darkMessages: false));

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void ALightAppDrawsEveryMessageLight(bool darkMessages) =>
        Assert.Equal(MessageScheme.Light, MessageSchemes.For(appDark: false, darkMessages));
}
