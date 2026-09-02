// The default-apps deep link (Services/DefaultMailApp.cs).
//
// Windows cannot be told to make this app the mail handler; it can only be asked to show the user
// the page where they decide (docs/os-integration.md). So the whole client-side decision is the
// URI, and it fails in the way a URI does: silently, landing on the wrong page or on nothing.
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class DefaultMailAppTests
{
    [Fact]
    public void TheDeepLinkNamesThisAppWhenItHasAnIdentity()
    {
        // `registeredAUMID` is the parameter for a packaged (MSIX) app; the other two are for an
        // installer that writes its own RegisteredApplications key, which this app has not.
        Assert.Equal(
            "ms-settings:defaultapps?registeredAUMID=Allodia.Mailcal_8wekyb3d8bbwe%21App",
            DefaultMailApp.SettingsUri("Allodia.Mailcal_8wekyb3d8bbwe!App"));
    }

    [Fact]
    public void AnAumidIsEscapedSoTheQueryStringSurvivesIt()
    {
        // An AUMID always contains `!` between the family name and the app id, and may contain
        // characters a query string would otherwise end at. Unescaped, Windows opens the Default
        // apps page without selecting anything, which reads as the button not working.
        var uri = DefaultMailApp.SettingsUri("A B_x!App&extra=1");
        Assert.DoesNotContain(" ", uri);
        Assert.DoesNotContain("!App&extra", uri);
        Assert.StartsWith("ms-settings:defaultapps?registeredAUMID=", uri);
    }

    [Fact]
    public void WithNoIdentityItFallsBackToThePageItself()
    {
        // The unpackaged dev build has no AUMID. A deep link naming an empty app would land
        // nowhere; the plain page is the honest answer, and the same one an older Windows gives.
        Assert.Equal("ms-settings:defaultapps", DefaultMailApp.SettingsUri(null));
        Assert.Equal("ms-settings:defaultapps", DefaultMailApp.SettingsUri(string.Empty));
    }

    [Fact]
    public void WindowsCanOnlyOpenSettingsAndNeverSetsTheHandlerItself()
    {
        // Not a tautology: it is the rule the core reads to decide what to offer. Windows has had
        // no API to set a default handler since Windows 10, by design, so a build here reporting
        // SetDirectly would put a button in front of the user that could not work.
        Assert.Equal(DefaultMailAppSupport.OpenSettings, DefaultMailApp.Support);
    }

    [Fact]
    public void WhetherWeAreAlreadyTheDefaultCannotBeToldHere()
    {
        // Reading the current association means reading a tamper-protected, undocumented UserChoice
        // key. The core treats null as "not the default", which is the recoverable way round.
        Assert.Null(DefaultMailApp.IsDefault);
    }
}
