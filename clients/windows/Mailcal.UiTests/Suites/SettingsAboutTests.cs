// Settings → About (docs/settings.md slot 11): the release this build is, where to ask for help, and
// the toolkit this client links. about_info is covered in Rust; what this suite is for is the client
// side, the category missing from the list, the panel never reached, or AboutPlatform.Windows copied
// from a sibling client as another platform's. Each hides About or attributes software the app does
// not ship, and every headless gate stays green through all three.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SettingsAboutTests(ShowcaseApp showcase)
{
    private void Case(string name, Action<App, SetupFlow> body) =>
        showcase.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    private static List<string> AboutTexts(App app, SetupFlow settings) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = settings.OpenCategory("about") }).Select(t => t.Name).ToList();

    [OfflineFact]
    public void SettingsOffersAboutAndOffersItLast() => Case(nameof(SettingsOffersAboutAndOffersItLast), (_, settings) =>
    {
        // Last, because it is what you go looking for rather than adjust (docs/settings.md).
        var categories = settings.Categories();
        Assert.True(categories.LastOrDefault() == "about", $"About comes last; the order is {string.Join(" | ", categories)}");
    });

    [OfflineFact]
    public void AboutNamesTheReleaseThisBuildIs() => Case(nameof(AboutNamesTheReleaseThisBuildIs), (app, settings) =>
    {
        // /VERSION, which check-version-sync keeps equal to the crate version, so this states the
        // contract rather than today's number: a support answer is built on it.
        var version = File.ReadAllText(Path.Combine(App.Repo, "VERSION")).Trim();
        var texts = AboutTexts(app, settings);
        Assert.True(texts.Contains(Catalog.English("about_version").Replace("{version}", version)),
            $"About quotes the release /VERSION holds ({version}); the panel reads: {string.Join(" / ", texts)}");
    });

    [OfflineFact]
    public void AboutAttributesTheToolkitThisClientActuallyLinksAndNoOther() => Case(nameof(AboutAttributesTheToolkitThisClientActuallyLinksAndNoOther), (app, settings) =>
    {
        var texts = AboutTexts(app, settings);
        var reads = string.Join(" / ", texts);
        Assert.True(texts.Contains("Rust"), $"every client ships the core, so Rust is attributed everywhere. The panel reads: {reads}");
        Assert.True(texts.Contains("Windows App SDK and WinUI"), $"an attribution this client does not show is a notice it has not given. The panel reads: {reads}");
        // Another client's AboutPlatform shows as somebody else's toolkit, not as an empty page.
        var foreign = texts.Where(t => System.Text.RegularExpressions.Regex.IsMatch(t, "GTK|libadwaita|WebKitGTK|AndroidX|Jetpack")).ToList();
        Assert.True(foreign.Count == 0, $"Windows links none of these, and attributing them is a false notice: {string.Join(" / ", foreign)}");
    });

    [OfflineFact]
    public void TheSupportAddressIsOnThePageWithAControlThatOpensIt() => Case(nameof(TheSupportAddressIsOnThePageWithAControlThatOpensIt), (app, settings) =>
    {
        var texts = AboutTexts(app, settings);
        Assert.True(texts.Contains("https://support.allodia.eu"), $"About exists so someone with a problem can find where to ask. The panel reads: {string.Join(" / ", texts)}");
        // A button, not the text inside it: the way out of the app is what a screen reader reaches for.
        var open = app.Agent.Find(new Filter { Type = "Button", Name = Catalog.English("about_support_action"), Under = settings.OpenCategory("about") });
        Assert.True(open is { Enabled: true }, "the address is selectable text, and the way to it is a button that can be pressed");
    });
}
