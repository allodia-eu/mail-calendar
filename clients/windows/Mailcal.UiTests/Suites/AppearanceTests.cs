// The app's own light or dark setting (docs/settings.md, General). Three things have to hold, and
// none shows without a drawn window:
//
//   * the choice reaches the window: painting it is a RequestedTheme on the content root, wiring
//     that compiles, renders and does nothing.
//   * a pick repaints at once: the core signals only Surface::Settings for it and computes nothing,
//     so no snapshot carries the change, and the dialog repaints the window itself. Cut that call
//     and the picker still moves and persists, and the window stays as it was.
//   * "Use system setting" hands the window back: ElementTheme.Default is what does it, and pinning
//     the theme any other way leaves the system's choice unreachable until the next launch.
//
// The launch is pinned to the scheme this desktop is not in, so every case measures the app's own
// decision. The showcase has no store, so a pick here is never written to the developer's
// preferences. The cases run in the order written: the last hands the window back.

using System.Drawing;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

public sealed class AppearanceApp : OppositeAppearanceApp;

[Trait("Dataset", "showcase")]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class AppearanceTests(AppearanceApp showcase) : IClassFixture<AppearanceApp>
{
    private static readonly bool DesktopDark = OppositeAppearanceApp.DesktopDark;

    // Nothing is near the midpoint: dark Mica draws around 20 to 30 and light around 170 to 240.
    private const double Mid = 128;

    private static string Scheme(bool dark) => dark ? "dark" : "light";

    /// <summary>Waits for the window, and with <paramref name="settings"/> the open dialog too, to
    /// read <paramref name="dark"/>: a pick repaints a frame or two later.</summary>
    private static void AssertDark(App app, bool dark, string because, bool settings = false)
    {
        double window = 0, dialog = 0;
        bool Holds()
        {
            window = App.CaptionLuminance(app.Mailbox.Hwnd);
            if (!settings)
            {
                return window < Mid == dark;
            }
            // An empty band of the dialog's title row.
            var open = app.Agent.Find(Filter.ById("SettingsDialog")) ?? throw new InvalidOperationException("Settings is not open");
            dialog = App.Luminance(open.Window, new Rectangle((int)(open.X + (open.Width * 0.62)), (int)open.Y + 18, 100, 8)).Median;
            return window < Mid == dark && dialog < Mid == dark;
        }
        App.Eventually(Holds, TimeSpan.FromSeconds(3));
        Assert.True(window < Mid == dark, $"the window sampled at luminance {window}, {Scheme(window < Mid)}, and must be {Scheme(dark)}: {because}");
        if (settings)
        {
            Assert.True(dialog < Mid == dark, $"Settings sampled at {dialog}, {Scheme(dialog < Mid)}, and must be {Scheme(dark)}: the open dialog follows the content root, not the theme it copied when it opened");
        }
    }

    /// <summary>Settings → General's three appearance choices, by their catalog labels.</summary>
    private static Dictionary<string, Element> Choices(App app)
    {
        var dialog = new SetupFlow(app).OpenCategory("general");
        return new[] { "settings_appearance_system", "settings_appearance_light", "settings_appearance_dark" }.ToDictionary(key => key, key =>
            app.Agent.Find(new Filter { Type = "RadioButton", Name = Catalog.English(key), Under = dialog })
            ?? throw new InvalidOperationException($"Settings → General offers no '{Catalog.English(key)}': all three are the contract (docs/settings.md)"));
    }

    [ShowcaseFact]
    public void TheAppPaintsItselfInItsOwnAppearanceNotTheDesktops() => showcase.Case(nameof(TheAppPaintsItselfInItsOwnAppearanceNotTheDesktops), app =>
        AssertDark(app, !DesktopDark,
            $"the run is pinned to {Scheme(!DesktopDark)} on a {Scheme(DesktopDark)} desktop; matching the desktop means the setting does not reach the content root"));

    [ShowcaseFact]
    public void SettingsGeneralOffersFollowTheSystemLightAndDark() => showcase.Case(nameof(SettingsGeneralOffersFollowTheSystemLightAndDark), app =>
        Assert.Equal(3, Choices(app).Count));

    [ShowcaseFact]
    public void PickingAnAppearanceRepaintsTheWindowAtOnceBothWays() => showcase.Case(nameof(PickingAnAppearanceRepaintsTheWindowAtOnceBothWays), app =>
    {
        var choices = Choices(app);
        app.Agent.Select(choices[DesktopDark ? "settings_appearance_dark" : "settings_appearance_light"]);
        AssertDark(app, DesktopDark, "the pick left the window as it was: nothing reloads a snapshot for it, so the dialog repaints the window itself", settings: true);
        app.Agent.Select(choices[DesktopDark ? "settings_appearance_light" : "settings_appearance_dark"]);
        AssertDark(app, !DesktopDark, "picking back has to move it back: a repaint that runs one way passes a one-way test", settings: true);
    });

    [ShowcaseFact]
    public void UseSystemSettingHandsTheWindowBackToTheDesktop() => showcase.Case(nameof(UseSystemSettingHandsTheWindowBackToTheDesktop), app =>
    {
        app.Agent.Select(Choices(app)["settings_appearance_system"]);
        AssertDark(app, DesktopDark,
            "following the host is reachable while the app runs, which is why the theme is set on the content root rather than pinned on the Application", settings: true);
    }, app => app.CloseDialogs());
}
