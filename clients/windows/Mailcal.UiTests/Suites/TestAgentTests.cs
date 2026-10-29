// The test agent's own promises, where a broken one fails some other suite in a way that names the
// wrong thing: a press aimed at "window 0" reads as a window that would not come to the front.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class TestAgentTests(ShowcaseApp showcase)
{
    [OfflineFact]
    public void AnElementKeepsItsWindowAcrossACollectionInTheApp() => showcase.Case(nameof(AnElementKeepsItsWindowAcrossACollectionInTheApp), app =>
    {
        // A list row's peer has no element of its own, so its window is found through its parent.
        // The agent holds peers weakly, and a collection replaces the managed wrapper it walked with
        // a new one for the same peer; the release probe collects every 250 ms while it runs.
        var row = app.Rows().First();
        Assert.Equal(app.Mailbox.Hwnd, row.Window);
        app.Agent.Collect();
        Assert.Equal(app.Mailbox.Hwnd, app.Agent.Read(row).Element.Window);
    });
}
