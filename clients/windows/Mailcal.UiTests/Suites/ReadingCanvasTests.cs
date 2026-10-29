// The reading pane's body area is the page a message is drawn on (docs/sync-progress.md), and in a
// dark window which page that is depends on the reader's choice (docs/reading-dark-mode.md): dark
// with dark messages on, white once the sun/moon button sends messages back to their light page.
//
//   * With dark messages on, the page is dark and the ink on it light: ReadingView.Scheme.cs sets
//     both from the core's message_canvas, and the theme over the page is the page's.
//   * After the button, the page is painted white inside a dark window. BodyArea.Background is
//     wiring that compiles and does nothing, and left unset the area shows the pane's own dark
//     background, a hole in a white page.
//   * On that white page the ink is resolved for the page: BodyArea.RequestedTheme does that, and
//     left at the window's, the dark theme's label colours draw white on white.
//
// Colour is a pixel rule, so the page is sampled off the screen. The launch is pinned dark, because
// a light window has no button and every page is white; the first case is what makes the pin mean
// something. The harness, because the body must be text/plain, drawn by WinUI rather than by a
// WebView2 document painting its own; the showcase seeds HTML bodies only.
//
// The preference persists, so the case that presses the button presses it again before it ends.

using System.Drawing;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(DarkHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class ReadingCanvasTests(DarkHarnessApp harness)
{
    private const string Plain = "Harness baseline message";

    // Nothing here is near the midpoint: a dark caption renders around 20 to 30, the light page is 255.
    private const double Mid = 128;

    // "This is the light page": the light theme's panel brushes sit in the 240s and the dark theme's
    // in the 30s and 40s.
    private const double PageFloor = 200;

    // "There is ink on it": light-theme body text is near black, and white on white leaves the
    // darkest pixel as light as the page.
    private const double InkCeiling = 100;

    // The dark page's ink floor: #e3e3e3 on #1e1e1e.
    private const double LightInkFloor = 170;

    private void Case(string name, Action<App> body, Action<App>? cleanup = null) => harness.Case(name, body, cleanup);

    /// <summary>The plain-text fixture open, and the control that draws it. PlainScroller, because
    /// a bare layout panel has no peer, so BodyArea itself cannot be measured.</summary>
    private static Element PlainBody(App app)
    {
        app.OpenMessage(Plain);
        var body = app.Agent.Wait(Filter.ById("PlainScroller") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException($"the reading pane never drew '{Plain}' as text");
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        return app.Steady(body);
    }

    /// <summary>The sun/moon button, which exists only in a dark window.</summary>
    private static Element Toggle(App app) =>
        app.Agent.Wait(Filter.ById("ReadingSchemeToggle", "Button") with { OnScreen = true }, TimeSpan.FromSeconds(15))
        ?? throw new InvalidOperationException("no ReadingSchemeToggle on the action row of a window pinned dark (docs/reading-dark-mode.md, rule 3)");

    /// <summary>The page behind the plain-text body, and the ink over its first lines.</summary>
    private static ((double Median, double Darkest, double Lightest) Page, (double Median, double Darkest, double Lightest) Ink) Sample(App app)
    {
        var body = PlainBody(app);
        var scale = Input.Scale(body.Window);
        // Inset well past the ScrollViewer's edge chrome and below the first lines, so the band is
        // the page and nothing else.
        var inset = (int)(40 * scale);
        var page = App.Luminance(body.Window, new Rectangle(
            (int)body.X + inset, (int)(body.Y + body.Height) - inset, (int)Math.Min(240, body.Width - (2 * inset)), 24));
        // The first lines: the ScrollViewer pads 16, and the text starts at its top left.
        var pad = (int)(16 * scale);
        var ink = App.Luminance(body.Window, new Rectangle(
            (int)body.X + pad, (int)body.Y + pad, (int)Math.Min(400, body.Width - (2 * pad)), (int)(44 * scale)));
        return (page, ink);
    }

    [HarnessFact]
    public void TheWindowReallyIsDarkWhichIsWhatMakesTheRestOfThisSuiteAnAssertion() => Case(nameof(TheWindowReallyIsDarkWhichIsWhatMakesTheRestOfThisSuiteAnAssertion), app =>
    {
        var caption = App.CaptionLuminance(app.Mailbox.Hwnd);
        Assert.True(caption < Mid,
            $"the caption sampled at {caption}, light, in a run pinned dark with MAILCAL_APPEARANCE; a light window has no sun/moon button, so every case below would prove nothing");
    });

    [HarnessFact]
    public void WithDarkMessagesOnThePageIsDarkAndTheInkOnItLight() => Case(nameof(WithDarkMessagesOnThePageIsDarkAndTheInkOnItLight), app =>
    {
        var (page, ink) = Sample(app);
        Assert.True(page.Median < Mid,
            $"the page behind a plain-text body sampled at {page.Median} in a dark window with dark messages on; it follows the scheme (docs/reading-dark-mode.md, rule 5)");
        Assert.True(ink.Lightest > LightInkFloor,
            $"the lightest pixel over the first lines is {ink.Lightest}, so the text is dark on a dark page: BodyArea.RequestedTheme is not following the page");
    });

    [HarnessFact]
    public void TheButtonOpensTheTrailingGroupBeforeArchive() => Case(nameof(TheButtonOpensTheTrailingGroupBeforeArchive), app =>
    {
        PlainBody(app);
        var toggle = Toggle(app);
        var forward = app.Agent.Find(Filter.ById("ReadingForward") with { OnScreen = true }) ?? throw new InvalidOperationException("no Forward on the row");
        var archive = app.Agent.Find(Filter.ById("ReadingArchive") with { OnScreen = true }) ?? throw new InvalidOperationException("no Archive on the row");
        Assert.True(forward.X < toggle.X && toggle.X < archive.X,
            $"Forward at {forward.X}, the button at {toggle.X}, Archive at {archive.X}: the button sits immediately before archive and delete (docs/reading-actions.md)");
    });

    [HarnessFact]
    public void TheSunSendsTheMessageToItsLightPagePaintedWithInkResolvedForIt()
    {
        var pressed = false;
        Case(nameof(TheSunSendsTheMessageToItsLightPagePaintedWithInkResolvedForIt), app =>
        {
            PlainBody(app);
            var sun = Toggle(app).Name;
            app.Agent.Invoke(Toggle(app));
            pressed = true;
            App.Eventually(() => Toggle(app).Name != sun, TimeSpan.FromSeconds(5));
            var (page, ink) = Sample(app);
            Assert.True(page.Median > PageFloor,
                $"after the sun, the body area sampled at {page.Median} in a dark window, the pane's own background showing through: BodyArea.Background is not the core's light canvas");
            Assert.True(ink.Darkest < InkCeiling,
                $"the darkest pixel over the first lines is {ink.Darkest}, so there is no ink on the white page: the text took the dark window's label colour");
            Assert.True(Toggle(app).Name != sun, $"the button still reads '{sun}' after it was pressed");
        }, app =>
        {
            // The preference persists: dark messages go back on for whatever runs next.
            if (pressed)
            {
                app.Agent.Invoke(Toggle(app));
            }
        });
    }
}
