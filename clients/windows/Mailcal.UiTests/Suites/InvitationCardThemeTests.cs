// The invitation card is legible in the appearance the app is in, not the desktop's. The card is
// built in code, so its colours are resolved per draw, and the obvious way is wrong:
// Application.Current.Resources answers for the application's theme, the desktop's, fixed before
// any window exists, while the setting is applied to the content root. Pinned light on a dark
// desktop, every label came back near-white on a light card (docs/client-traps.md;
// Services/ThemePalette.cs).
//
// The card draws perfectly in every other respect, and a foreground brush is not an automation
// property, so every other suite passes over an unreadable card; AppearanceTests samples the
// caption, which is XAML and was always right. So the launch is pinned to the scheme this desktop is
// not in, and each line is read off the screen: its ink contrasts with its own ground, on the side
// the theme demands. A relation rather than a value, so no Fluent constant is repeated here.
//
// The card is reached through its preview expander, found by class, and its lines as texts, so this
// says the same in every language.

using System.Drawing;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

public sealed class InvitationCardThemeApp : OppositeAppearanceApp;

[Trait("Dataset", "showcase")]
public sealed class InvitationCardThemeTests(InvitationCardThemeApp showcase) : IClassFixture<InvitationCardThemeApp>
{
    // A glyph's core differs from its ground by over a hundred when it is right; the failure this is
    // for sat at 4, with the sign inverted.
    private const double MinContrast = 40;

    /// <summary>
    /// The card's lines: every text under the card's nearest ancestor with a peer (its own panels
    /// have none), minus the preview's, whose event blocks are light on a saturated fill in both
    /// themes and would answer with the opposite sign.
    /// </summary>
    private static List<Element> CardLines(App app)
    {
        // The English seed's invitation; pressing the row already open would close it.
        var heading = Filter.ById("ReadingSubject") with { Window = app.Mailbox.Hwnd, NameLike = "Invitation:*" };
        if (app.Agent.Find(heading) is null)
        {
            app.Agent.Invoke(app.RowTitled("Invitation:*"));
            app.Agent.Wait(heading, App.Patience);
        }
        // The card builds a new expander on every redraw, and the calendar's arrival redraws it, so
        // it is read once it has stopped changing: an old expander's peer refuses to be read.
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(600), floor: TimeSpan.FromSeconds(1), cap: TimeSpan.FromSeconds(5));
        IReadOnlyList<Element> expanders = [];
        App.Eventually(() => (expanders = app.Agent.FindAll(new Filter { Window = app.Mailbox.Hwnd, OnScreen = true })
            .Where(e => e.Class == "Microsoft.UI.Xaml.Controls.Expander").ToList()).Count > 0, App.Patience);
        if (expanders.Count != 1)
        {
            throw new InvalidOperationException(expanders.Count == 0
                ? "the showcase invitation drew no card; the showcase primes the calendar at boot so the preview opens (boot/inmemory.rs)"
                : $"expected one expander in the reading pane, found {expanders.Count}: something is open over it, and the lines would be that surface's");
        }
        var preview = expanders[0];
        var card = app.Agent.Parent(preview) ?? throw new InvalidOperationException("the preview has no ancestor with a peer");
        bool InPreview(Element e) => e.X >= preview.X && e.X < preview.X + preview.Width && e.Y >= preview.Y && e.Y < preview.Y + preview.Height;
        return app.Agent.FindAll(new Filter { Type = "Text", Under = card, OnScreen = true })
            .Where(t => t.Name.Trim().Length > 0 && t.Width >= 4 && t.Height >= 4 && !InPreview(t))
            .ToList();
    }

    /// <summary>A line's ink and ground: most pixels in a text's rectangle are ground, so the median
    /// is the ground, and the ink is whichever extreme lies further from it.</summary>
    private static (double Ink, double Ground) Read(Element line)
    {
        var (median, darkest, lightest) = App.Luminance(line.Window, new Rectangle((int)line.X, (int)line.Y, (int)line.Width, (int)line.Height));
        return (median - darkest >= lightest - median ? darkest : lightest, median);
    }

    [OfflineFact]
    public void EveryLineOfTheInvitationCardHasInkAgainstItsOwnGround() => showcase.Case(nameof(EveryLineOfTheInvitationCardHasInkAgainstItsOwnGround), app =>
    {
        var lines = CardLines(app);
        Assert.True(lines.Count >= 4, $"the card carries at least its title and the organiser, when and where rows; found {lines.Count} line(s), so this would measure the wrong element");
        foreach (var line in lines)
        {
            var (ink, ground) = Read(line);
            Assert.True(Math.Abs(ink - ground) >= MinContrast,
                $"'{line.Name}' is drawn at luma {ink} on {ground}, {Math.Abs(ink - ground):0} apart against a floor of {MinContrast}: a colour from the wrong theme, correctly worded and placed, and unreadable");
        }
    });

    [OfflineFact]
    public void AndTheInkIsOnTheSideThisAppearanceDemandsNotTheDesktops() => showcase.Case(nameof(AndTheInkIsOnTheSideThisAppearanceDemandsNotTheDesktops), app =>
    {
        var darkApp = !OppositeAppearanceApp.DesktopDark;
        foreach (var line in CardLines(app))
        {
            var (ink, ground) = Read(line);
            Assert.True(ink > ground == darkApp,
                $"the run is pinned {(darkApp ? "dark" : "light")}, so ink is {(darkApp ? "lighter" : "darker")} than its ground; '{line.Name}' came back the other way (ink {ink}, ground {ground}): the application's theme leaking into a code-built surface");
        }
    });
}
