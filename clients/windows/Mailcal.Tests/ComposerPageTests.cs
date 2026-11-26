// The page the shared editor writes its message on (docs/reading-dark-mode.md, "Writing in a dark
// app"): what is handed over before the editor's document runs, and the order a host hands the core
// what the editor posts. The core's scripts are stood in for, since no cdylib is loaded here.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ComposerPageTests
{
    private static string Page(MessageScheme scheme) => $"page({scheme});";

    [Fact]
    public void ThePreloadNamesTheChromeTheAppsSchemeAndTheMessageItsPage()
    {
        Assert.Equal(
            "window.composerScheme = \"dark\";page(Dark);",
            ComposerPage.Preload(appDark: true, MessageScheme.Dark, Page));
        // A reader who keeps messages light writes on a light page in a dark app's chrome.
        Assert.Equal(
            "window.composerScheme = \"dark\";page(Light);",
            ComposerPage.Preload(appDark: true, MessageScheme.Light, Page));
        Assert.Equal(
            "window.composerScheme = \"light\";page(Light);",
            ComposerPage.Preload(appDark: false, MessageScheme.Light, Page));
    }

    [Fact]
    public void AHostThatNamesNoPageHandsOverTheChromeAlone() =>
        Assert.Equal(
            "window.composerScheme = \"dark\";",
            ComposerPage.Preload(appDark: true, page: null, Page));

    [Fact]
    public void AColoursRequestIsAnsweredAndGoesNoFurther()
    {
        var asked = false;
        var route = ComposerPage.Route(
            "colours",
            message => message == "colours" ? "answer();" : null,
            _ => asked = true,
            out var answer);
        Assert.Equal(EditorPost.Answered, route);
        Assert.Equal("answer();", answer);
        Assert.False(asked);
    }

    [Fact]
    public void TheToolbarsSunOrMoonIsTheCoresAndGoesNoFurther()
    {
        var route = ComposerPage.Route("background", _ => null, message => message == "background", out var answer);
        Assert.Equal(EditorPost.Chosen, route);
        Assert.Null(answer);
    }

    [Fact]
    public void AnythingElseIsLeftForTheHostsOwnRequests()
    {
        Assert.Equal(EditorPost.Request, ComposerPage.Route("link", _ => null, _ => false, out _));
        // A host that names no page has no sun or moon to take.
        Assert.Equal(EditorPost.Request, ComposerPage.Route("background", _ => null, choose: null, out _));
    }
}
