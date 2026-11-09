// The composer's formatting toolbar is drawn in the app's light or dark, and the page under it stays
// light either way (docs/settings.md, General). WebView2's own `prefers-color-scheme` follows the
// desktop rather than the app, so the toolbar follows the app only because EditorWebView.cs names
// the element's ActualTheme to the bundle on load and on every change.
//
// Colour is a pixel rule, so the editor is sampled off the screen. The launch is pinned to the
// appearance this desktop is not in: a toolbar that followed the desktop would pass a run pinned to
// the desktop's own. Nothing is sent; each composer is discarded.

using System.Drawing;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>Its own launch, because a case here picks an appearance in Settings.</summary>
public sealed class ComposerSchemeApp : OppositeAppearanceApp;

[Trait("Dataset", "showcase")]
public sealed class ComposerSchemeTests(ComposerSchemeApp showcase) : IClassFixture<ComposerSchemeApp>
{
    private static readonly bool DesktopDark = OppositeAppearanceApp.DesktopDark;

    // The toolbar's two colours are #fafafa and #252528 (editor.css), far either side of this.
    private const double Mid = 128;

    // "This is the light page": nothing in either scheme's chrome is this light but #fafafa, and the
    // page is sampled where no toolbar is.
    private const double PageFloor = 200;

    private static string Scheme(bool dark) => dark ? "dark" : "light";

    private void Case(string name, Action<App> body) =>
        showcase.Case(name, app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            body(app);
        }, app =>
        {
            app.CloseDialogs();
            app.CloseComposer();
        });

    /// <summary>The toolbar's colour and the page's, read off the screen.</summary>
    private static (double Toolbar, double Page) Sample(App app)
    {
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var editor = app.Steady(app.Agent.Find(Filter.ById("Editor") with { OnScreen = true })
            ?? throw new InvalidOperationException("the composer draws no #Editor"));
        var scale = Input.Scale(editor.Window);
        var inset = (int)(40 * scale);
        var width = (int)Math.Min(400, editor.Width - (2 * inset));
        // The toolbar's top padding, above its controls, so the band is the bar and nothing on it.
        var toolbar = App.Luminance(editor.Window, new Rectangle(
            (int)editor.X + inset, (int)(editor.Y + scale), width, Math.Max(2, (int)(2 * scale)))).Median;
        // Well below the toolbar and the placeholder line, inside the empty page.
        var page = App.Luminance(editor.Window, new Rectangle(
            (int)editor.X + inset, (int)(editor.Y + (editor.Height * 0.65)), width, (int)(24 * scale))).Median;
        return (toolbar, page);
    }

    private static void AssertScheme(App app, bool dark, string because)
    {
        var (toolbar, page) = (0d, 0d);
        App.Eventually(() =>
        {
            (toolbar, page) = Sample(app);
            return toolbar < Mid == dark;
        }, TimeSpan.FromSeconds(5));
        Assert.True(toolbar < Mid == dark,
            $"the toolbar sampled at luminance {toolbar}, {Scheme(toolbar < Mid)}, and must be {Scheme(dark)}: {because}");
        Assert.True(page > PageFloor,
            $"the page sampled at {page} under a {Scheme(dark)} toolbar; the page stays light in either scheme (docs/settings.md)");
    }

    /// <summary>Picks <paramref name="dark"/> in Settings → General and closes Settings, so nothing
    /// lies over the composer when it is sampled.</summary>
    private static void Pick(App app, bool dark)
    {
        var dialog = new SetupFlow(app).OpenCategory("general");
        var key = dark ? "settings_appearance_dark" : "settings_appearance_light";
        app.Agent.Select(app.Agent.Find(new Filter { Type = "RadioButton", Name = Catalog.English(key), Under = dialog })
            ?? throw new InvalidOperationException($"Settings → General offers no '{Catalog.English(key)}'"));
        app.CloseDialogs();
    }

    [OfflineFact]
    public void TheToolbarIsInTheAppsAppearanceNotTheDesktopsAndThePageStaysLight() =>
        Case(nameof(TheToolbarIsInTheAppsAppearanceNotTheDesktopsAndThePageStaysLight), app =>
            AssertScheme(app, !DesktopDark,
                $"the run is pinned to {Scheme(!DesktopDark)} on a {Scheme(DesktopDark)} desktop; matching the desktop means the editor was never told the app's scheme"));

    [OfflineFact]
    public void PickingAnAppearanceRepaintsAnOpenComposersToolbarBothWays() =>
        Case(nameof(PickingAnAppearanceRepaintsAnOpenComposersToolbarBothWays), app =>
        {
            AssertScheme(app, !DesktopDark, "the composer opened in the launch's appearance");
            Pick(app, DesktopDark);
            AssertScheme(app, DesktopDark, "the pick reached the window but not the open editor: ActualThemeChanged is not passed on");
            Pick(app, !DesktopDark);
            AssertScheme(app, !DesktopDark, "picking back has to move it back: a repaint that runs one way passes a one-way test");
        });
}
