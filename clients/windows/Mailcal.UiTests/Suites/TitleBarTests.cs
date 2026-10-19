// The window's caption is the app's own, a WinUI TitleBar in the content tree. Drawn by the system,
// it kept its own theme, and a dark app shipped with a pale strip across its top; nothing else in
// the repo could see that. So: the caption is the app's and names it; its background is on the
// desktop's side of light and dark, a pixel rule; the pane toggle lives in it and works, a forwarded
// handler of the kind that compiles and does nothing; and the system's minimise, maximise and close
// are as tall as it, which they are only when the window asks. The caption is also where the search
// field lives, centred over the window (docs/search.md), which is geometry.
//
// The launch follows the desktop's appearance (MAILCAL_APPEARANCE=system): AppearanceTests owns the
// app's own setting, and this is about the caption following whatever the app resolved to.

using System.Drawing;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase following the desktop's appearance.</summary>
public sealed class SystemAppearanceApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_APPEARANCE"] = "system" };
}

[Trait("Dataset", "showcase")]
public sealed class TitleBarTests(SystemAppearanceApp showcase) : IClassFixture<SystemAppearanceApp>
{
    // Dark Mica draws around 30 and light around 240: a caption on the wrong side is off by two hundred.
    private const double Mid = 128;

    /// <summary>The caption control. A search scoped to a missing one must stop the case rather than
    /// widen to the window, where it finds the NavigationView's own toggle.</summary>
    private static Element Caption(App app) =>
        app.Agent.Find(Filter.ById("AppTitleBar") with { Window = app.Mailbox.Hwnd })
        ?? throw new InvalidOperationException("no #AppTitleBar: without the app's own TitleBar the system draws a caption that ignores the app's theme");

    /// <summary>The search field, a sibling of the caption control rather than its content
    /// (MainWindow.xaml says why).</summary>
    private static Element SearchField(App app) =>
        app.Agent.Find(Filter.ById("SearchBox") with { Window = app.Mailbox.Hwnd }) is { Rendered: true } field
            ? app.Steady(field)
            : throw new InvalidOperationException("the window's top row carries no search field (docs/search.md)");

    /// <summary>The sidebar's first entry's width: open, it spans the pane; collapsed, the icon strip.</summary>
    private static double PaneItemWidth(App app) =>
        app.Steady(app.Agent.Find(Filter.Named(Catalog.English("sidebar_all_accounts"), "ListItem") with { Window = app.Mailbox.Hwnd })
            ?? throw new InvalidOperationException("the sidebar has no All Accounts entry")).Width;

    [ShowcaseFact]
    public void TheCaptionIsTheAppsOwnTitleBarCarryingTheAppName() => showcase.Case(nameof(TheCaptionIsTheAppsOwnTitleBarCarryingTheAppName), app =>
    {
        // The name this build was given, never the brand's literal: CI and every fork are unbranded.
        var title = Catalog.AppTitle();
        Assert.True(app.Agent.Find(new Filter { Name = title, Under = Caption(app) }) is not null,
            $"the caption names the app ('{title}'): it is what tells one window from another in Alt-Tab and the taskbar");
    });

    [ShowcaseFact]
    public void TheCaptionIsDrawnInTheDesktopsOwnLightDarkMode() => showcase.Case(nameof(TheCaptionIsDrawnInTheDesktopsOwnLightDarkMode), app =>
    {
        // Sampled in the gap between the search field and the caption buttons, bare caption in every
        // locale. From the field rather than a fraction of the window: the field is centred and its
        // width follows the window, and a sample over a control reads that control's fill.
        var window = Desktop.Bounds(app.Mailbox.Hwnd);
        var field = SearchField(app);
        var x = (int)(field.X + field.Width) + 24;
        if (x + 40 > window.Right)
        {
            throw new InvalidOperationException($"no bare caption to sample: the search field ends at {field.X + field.Width} and the window at {window.Right}");
        }
        var luminance = App.Luminance(app.Mailbox.Hwnd, new Rectangle(x, window.Y + 12, 40, 8)).Median;
        var dark = Desktop.Dark();
        Assert.True(luminance < Mid == dark,
            $"the desktop is {(dark ? "dark" : "light")} and the caption sampled at {luminance}: a caption that keeps its own theme draws a pale bar over a dark app, or the reverse");
    });

    [ShowcaseFact]
    public void TheCaptionButtonsAreAsTallAsTheCaptionTheySitIn() => showcase.Case(nameof(TheCaptionButtonsAreAsTallAsTheCaptionTheySitIn), app =>
    {
        // The one part of the caption the app does not draw, so it follows the caption only when the
        // window asks: otherwise 32 epx against 48, a third of a row above the icon and name.
        var expected = Desktop.CaptionHeightDip() * Input.Scale(app.Mailbox.Hwnd);
        foreach (var (id, bounds) in Desktop.CaptionButtons(app.Mailbox.Hwnd))
        {
            Assert.True(Math.Abs(bounds.Height - expected) <= 1, $"the {id} button is {bounds.Height}px tall against a caption of {expected}px");
        }
    });

    [ShowcaseFact]
    public void ThePaneToggleLivesInTheCaptionAndNowhereElse() => showcase.Case(nameof(ThePaneToggleLivesInTheCaptionAndNowhereElse), app =>
    {
        var buttons = app.Agent.FindAll(new Filter { Type = "Button", Under = Caption(app) }).Select(b => b.Id).ToList();
        Assert.True(buttons.SequenceEqual([Filter.ById("PART_PaneToggleButton").Id!]),
            $"the caption control carries one button, the pane toggle; anything else is a stray affordance. Found: {string.Join(" | ", buttons)}");
        // The NavigationView's own is hidden: two hamburgers read as two controls.
        Assert.True(app.Agent.Find(Filter.ById("TogglePaneButton")) is null, "the NavigationView draws no pane toggle of its own as well");
    });

    [ShowcaseFact]
    public void TheCaptionsPaneToggleCollapsesAndReopensTheSidebar() => showcase.Case(nameof(TheCaptionsPaneToggleCollapsesAndReopensTheSidebar), app =>
    {
        var toggle = app.Agent.Find(new Filter { Id = "PART_PaneToggleButton", Type = "Button", Under = Caption(app) })
            ?? throw new InvalidOperationException("the caption carries no pane toggle");
        var open = PaneItemWidth(app);
        app.Agent.Invoke(toggle);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        var collapsed = PaneItemWidth(app);
        Assert.True(collapsed < open, $"the sidebar entry measured {collapsed}px after the toggle against {open}px open: its PaneToggleRequested handler is the only thing that closes the pane");
        app.Agent.Invoke(toggle);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        Assert.Equal(open, PaneItemWidth(app));
    });

    [ShowcaseFact]
    public void TheSearchFieldIsCentredOverTheWindowNotOverAPane() => showcase.Case(nameof(TheSearchFieldIsCentredOverTheWindowNotOverAPane), app =>
    {
        // The default scope is every account and folder, so a field over the message list reached
        // wider than the column it sat on. Tight, because the TitleBar's own centre slot is off by
        // tens of pixels.
        var window = Desktop.Bounds(app.Mailbox.Hwnd);
        var field = SearchField(app);
        var fieldCentre = field.X + (field.Width / 2);
        var windowCentre = window.X + (window.Width / 2.0);
        Assert.True(Math.Abs(fieldCentre - windowCentre) <= 2, $"the search field's centre ({fieldCentre}) is the window's ({windowCentre})");
        // Two fields for one query is worse than either placement.
        Assert.True(app.Agent.Find(new Filter { Type = "Edit", Under = app.RowsList() }) is null, "the message list draws no search field of its own");
    });
}
