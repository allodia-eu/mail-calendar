// The composer writes its message on the page a reading view would draw it on
// (docs/reading-dark-mode.md, "Writing in a dark app"): dark in a dark app while dark messages are
// on, from the editor's first frame, and following the preference wherever it is changed. The page
// is the core's; what this suite watches is that EditorWebView.cs hands it over before the document
// runs, names it again on every change of the preference, and takes the toolbar's sun or moon to
// the core.
//
// Colour is a pixel rule, so the page is sampled off the screen. The launch is pinned dark, since a
// light app writes on a light page whatever the preference says. The web view's own background is
// transparent, so a frame it has not painted shows the dark pane: only a light sample is a light
// page. The preference is the one the reading view's button sets, so a case that moves it puts it
// back. Nothing is sent; each composer is discarded.

using System.Drawing;
using System.Windows.Automation;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase pinned dark, so a message has a dark page to be written on.</summary>
public sealed class DarkShowcaseApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_APPEARANCE"] = "dark" };
}

[Trait("Dataset", "showcase")]
public sealed class ComposerDarkPageTests(DarkShowcaseApp showcase) : IClassFixture<DarkShowcaseApp>
{
    // The dark page is #1e1e1e and the light one white, far either side of this.
    private const double Mid = 128;

    // "This is the light page".
    private const double PageFloor = 200;

    private void Case(string name, Action<App> body, Action<App>? cleanup = null) =>
        showcase.Case(name, body, app =>
        {
            app.CloseDialogs();
            cleanup?.Invoke(app);
            app.CloseComposer();
        });

    /// <summary>The page's colour, well below the toolbar and the placeholder line.</summary>
    private static double Page(Element editor)
    {
        var scale = Input.Scale(editor.Window);
        var inset = (int)(40 * scale);
        var width = (int)Math.Min(400, editor.Width - (2 * inset));
        return App.Luminance(editor.Window, new Rectangle(
            (int)editor.X + inset, (int)(editor.Y + (editor.Height * 0.65)), width, (int)(24 * scale))).Median;
    }

    private static Element Editor(App app) =>
        app.Steady(app.Agent.Find(Filter.ById("Editor") with { OnScreen = true })
            ?? throw new InvalidOperationException("the composer draws no #Editor"));

    private static void AssertPage(App app, bool dark, string because)
    {
        var page = 0d;
        App.Eventually(() => (page = Page(Editor(app))) < Mid == dark, TimeSpan.FromSeconds(10));
        Assert.True(page < Mid == dark, $"the composer's page sampled at {page}: {because}");
    }

    /// <summary>Sets Settings → Reading's dark-messages switch and closes Settings, so nothing lies
    /// over the composer when it is sampled.</summary>
    private static void SetDarkMessages(App app, bool on)
    {
        var dialog = new SetupFlow(app).OpenCategory("reading");
        app.Agent.SetToggle(app.Agent.Find(new Filter { Id = "SettingsDarkMessages", Under = dialog })
            ?? throw new InvalidOperationException("Settings → Reading has no SettingsDarkMessages switch"), on);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        app.CloseDialogs();
    }

    /// <summary>A button in the editor's own page, read through UI Automation.</summary>
    private static AutomationElement? EditorButton(string name, TimeSpan patience)
    {
        var named = new AndCondition(
            new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Button),
            new PropertyCondition(AutomationElement.NameProperty, name));
        AutomationElement? button = null;
        App.Eventually(() => (button = EditorWindows.Browser()
            .Select(w => w.FindFirst(TreeScope.Descendants, named)).FirstOrDefault(b => b is not null)) is not null,
            patience);
        return button;
    }

    /// <summary>Presses a button in the page. One that opens a menu (`aria-haspopup`) is reached as
    /// an expander, with no Invoke.</summary>
    private static void Press(AutomationElement button)
    {
        if (button.TryGetCurrentPattern(InvokePattern.Pattern, out var invoke))
        {
            ((InvokePattern)invoke).Invoke();
            return;
        }
        ((ExpandCollapsePattern)button.GetCurrentPattern(ExpandCollapsePattern.Pattern)).Expand();
    }

    /// <summary>Presses the toolbar's sun or moon, named as the reading view's button is. It is the
    /// last control on the row, so a composer narrower than the row keeps it behind ⋯
    /// (docs/composer-layout.md, rule 7), which is opened first.</summary>
    private static void PressPageToggle(string key)
    {
        var name = Catalog.English(key);
        if (EditorButton(name, TimeSpan.FromSeconds(3)) is not { } toggle)
        {
            Press(EditorButton(Catalog.English("editor_more_formatting"), TimeSpan.FromSeconds(10))
                ?? throw new InvalidOperationException("the composer's toolbar has no ⋯ and no sun or moon on its row"));
            toggle = EditorButton(name, TimeSpan.FromSeconds(10))
                ?? throw new InvalidOperationException(
                    $"the composer's toolbar has no '{name}' button on its row or behind ⋯ in a dark app (docs/reading-dark-mode.md, rule 14)");
        }
        Press(toggle);
    }

    [OfflineFact]
    public void TheMessageIsOnTheDarkPageFromTheEditorsFirstFrame() =>
        Case(nameof(TheMessageIsOnTheDarkPageFromTheEditorsFirstFrame), app =>
        {
            var compose = app.Agent.Wait(Filter.ById("ComposeButton"), App.Patience)
                ?? throw new InvalidOperationException("no ComposeButton within 30 s");
            app.Agent.Invoke(compose);
            // Every frame that can be caught from the press until the editor has seeded and drawn
            // a while after: a page named only once the document has run paints it light first.
            var lightest = 0d;
            var samples = 0;
            var until = DateTime.UtcNow + TimeSpan.FromSeconds(4);
            while (DateTime.UtcNow < until)
            {
                if (app.Agent.Find(Filter.ById("Editor") with { OnScreen = true }) is { Width: > 100, Height: > 100 } editor)
                {
                    lightest = Math.Max(lightest, Page(editor));
                    samples++;
                }
            }
            new DraftsFlow(app).WaitEditorSeeded();
            Assert.True(samples > 5, $"only {samples} samples of the editor while it opened; the case saw nothing");
            Assert.True(lightest < Mid,
                $"the page sampled as light as {lightest} while the composer opened in a dark app: the editor drew a frame before it was on the page it was handed");
            AssertPage(app, dark: true, "it stays on the dark page once seeded");
        });

    [OfflineFact]
    public void TheOpenComposerFollowsSettingsReadingBothWays()
    {
        var moved = false;
        Case(nameof(TheOpenComposerFollowsSettingsReadingBothWays), app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            AssertPage(app, dark: true, "a dark app with dark messages on writes on the dark page");
            SetDarkMessages(app, on: false);
            moved = true;
            AssertPage(app, dark: false, "dark messages were switched off with the composer open, and its page did not follow");
            SetDarkMessages(app, on: true);
            moved = false;
            AssertPage(app, dark: true, "switching back has to move it back: a repaint that runs one way passes a one-way test");
        }, app =>
        {
            if (moved)
            {
                SetDarkMessages(app, on: true);
            }
        });
    }

    [OfflineFact]
    public void TheToolbarsSunSetsTheOnePreferenceAndItsMoonPutsItBack()
    {
        var moved = false;
        Case(nameof(TheToolbarsSunSetsTheOnePreferenceAndItsMoonPutsItBack), app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            AssertPage(app, dark: true, "a dark app with dark messages on writes on the dark page");
            PressPageToggle("reading_show_light_background");
            moved = true;
            AssertPage(app, dark: false, "the sun was pressed and the page stayed dark: the choice did not reach the core, or its Settings signal did not name the page back");
            // The preference the reading view and Settings show, not a page of the composer's own.
            var dialog = new SetupFlow(app).OpenCategory("reading");
            var toggle = app.Agent.Find(new Filter { Id = "SettingsDarkMessages", Under = dialog })
                ?? throw new InvalidOperationException("Settings → Reading has no SettingsDarkMessages switch");
            Assert.True(app.Agent.Read(toggle).Active == false,"the composer's sun moved its page but not Settings → Reading's switch: it is not the one preference");
            app.CloseDialogs();
            PressPageToggle("reading_show_dark_background");
            moved = false;
            AssertPage(app, dark: true, "the moon has to put the dark page back");
        }, app =>
        {
            if (moved)
            {
                SetDarkMessages(app, on: true);
            }
        });
    }
}

/// <summary>The WebView2 runtime's own top-level windows, where the editor's page is read.</summary>
internal static class EditorWindows
{
    internal static IEnumerable<AutomationElement> Browser() =>
        AutomationElement.RootElement.FindAll(TreeScope.Children, Condition.TrueCondition).Cast<AutomationElement>()
            .Where(w => w.Current.ClassName.StartsWith("Chrome_WidgetWin", StringComparison.Ordinal) && ProcessName(w.Current.ProcessId) == "msedgewebview2");

    private static string ProcessName(int id)
    {
        try
        {
            using var process = System.Diagnostics.Process.GetProcessById(id);
            return process.ProcessName;
        }
        catch (ArgumentException)
        {
            return "";
        }
    }
}
