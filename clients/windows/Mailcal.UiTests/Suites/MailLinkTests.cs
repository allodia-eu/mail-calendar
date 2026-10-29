// A mailto: link opens the composer, filled with what the link named and nothing else
// (docs/composer-security.md, Gate 12). Mailcal.Tests reaches the gate that decides an activation is
// a mail link, and the core's suite the decode; neither sees the decoded fields reach the composer's
// controls on the real activation path, where a link would arrive as a blank composer.
//
// The link goes in as the shell hands one over: a second process, which the single-instanced app
// redirects into the running one (Program.OnActivated). Nothing is sent; each composer is discarded.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed partial class MailLinkTests(ShowcaseApp showcase)
{
    private const string Link = "mailto:bob@example.com?subject=Lunch%20on%20Friday&cc=carol@example.com&bcc=dave@example.com";

    // The same link with headers a link may never set. `from` matters most: honoured, any web page
    // would decide who a message appears to come from.
    private const string SpoofedLink = "mailto:bob@example.com?from=spoof@evil.test&reply-to=spoof@evil.test&subject=Hi";

    // A pill's remove button is named "<address>, Remove recipient", so it names the recipient.
    [GeneratedRegex(@"^(?<address>[^\s@,]+@[^\s@,]+),\s")]
    private static partial Regex Pill();

    private void Case(string name, Action<App> body) => showcase.Case(name, body, app => app.CloseComposer());

    private static void OpenLink(App app, string link)
    {
        app.HandOver(link);
        if (app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of the mail link: the activation never reached the window, or was dropped before opening one");
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
    }

    /// <summary>
    /// The recipients the composer holds, top first. The three fields are one control used three
    /// times, so a pill looks the same in each; they are stacked To, Cc, Bcc, so top to bottom is
    /// the field order, which makes "the Bcc landed in Bcc" something this can fail on. The header
    /// fields scroll in a box capped at part of the composer (ComposerView.Layout.cs), so on a
    /// short pane Bcc is below its fold, where a pill reports no position: the pills are read at the
    /// top and again at the end, and each must be drawn in one of the two.
    /// </summary>
    private static List<string> Recipients(App app)
    {
        var header = app.Agent.Find(Filter.ById("HeaderScroll")) ?? throw new InvalidOperationException("the composer has no header scroller (#HeaderScroll)");
        List<string> Drawn(double percent)
        {
            app.Agent.ScrollTo(header, percent);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
            return app.Agent.FindAll(new Filter { Type = "Button" })
                .Select(b => (Button: b, Match: Pill().Match(b.Name)))
                .Where(p => p.Match.Success && p.Button.Rendered)
                .OrderBy(p => p.Button.Y)
                .Select(p => p.Match.Groups["address"].Value)
                .ToList();
        }
        var top = Drawn(0);
        var seen = top.Concat(Drawn(100).Where(address => !top.Contains(address))).ToList();
        Drawn(0);
        return seen;
    }

    [OfflineFact]
    public void AMailLinkOpensTheComposerWithEveryFieldItNamed() => Case(nameof(AMailLinkOpensTheComposerWithEveryFieldItNamed), app =>
    {
        OpenLink(app, Link);
        // Cc and Bcc arrive and are on screen: a recipient the user cannot see before Send is one
        // they cannot remove, which is why Gate 12 makes a pre-filled Bcc a visibility rule.
        Assert.Equal("bob@example.com, carol@example.com, dave@example.com", string.Join(", ", Recipients(app)));
        Assert.Equal("Lunch on Friday", app.Agent.Read(app.Agent.Find(Filter.ById("SubjectBox"))!).Value);
    });

    [OfflineFact]
    public void ALinkCannotDictateWhoTheMessageComesFrom() => Case(nameof(ALinkCannotDictateWhoTheMessageComesFrom), app =>
    {
        // The allowlist is the core's; the client is where it would be undone, by some other
        // decoder on the way to the fields. Names and text-box values both: a text box's name is its
        // header, never its content, so names alone read a spoofed subject as clean.
        OpenLink(app, SpoofedLink);
        var names = app.Agent.FindAll(new Filter()).Select(e => e.Name);
        var values = app.Agent.FindAll(new Filter { Type = "Edit" }).Select(e => app.Agent.Read(e).Value ?? "");
        var spoofed = names.Concat(values).Where(text => text.Contains("spoof@evil.test", StringComparison.Ordinal)).ToList();
        Assert.True(spoofed.Count == 0, $"a header the link may never set reached the composer: {string.Join(" | ", spoofed)}");
        Assert.Equal("bob@example.com", string.Join(", ", Recipients(app)));
    });
}
