// What the three status-line suites share (docs/sync-progress.md): the caption a staged state puts
// in the footer, and the rule that it shares the footer's line rather than opening a row of its own.
//
// Each stages its state in the core with a MAILCAL_FAKE_SYNC_* switch (debug builds only), which
// substitutes the snapshot the host reads and nothing else: the surface signal, the bound
// properties and the layout pass are real. A hook that set the model's property instead would keep
// passing after the core-to-client wiring was cut. A switch is read once at startup, so each state
// is a launch, and a suite, of its own.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

internal static class SyncStatusFlow
{
    /// <summary>The showcase's primary account. Its id is its address (boot/inmemory.rs mints the
    /// id from the identity), so the caption naming it proves the client resolved the id.</summary>
    internal const string Account = "eva.jansen@example.com";

    /// <summary>The staged caption, drawn, once the window has read the surface.</summary>
    internal static Element Caption(App app)
    {
        var caption = app.Agent.Wait(Filter.ById("SyncStatus") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException(
                "no SyncStatus on screen within 30 s: the staged state never reached the window (a Release build compiles the switch out; app.log warns whenever it is in force), or SyncStatusVisible is not bound");
        return caption.Rendered ? caption : throw new InvalidOperationException($"the status caption is in the tree but not drawn ({caption.Width}x{caption.Height})");
    }

    private static Element Drawn(App app, string id) =>
        app.Agent.Find(Filter.ById(id) with { OnScreen = true }) is { Rendered: true } element
            ? element
            : throw new InvalidOperationException($"#{id} is not drawn");

    /// <summary>
    /// The caption shares the footer's status line with "Connected": their centres agree, and it
    /// sits below the list. A caption on a row of its own would resize the list for work the user
    /// never started.
    /// </summary>
    internal static void AssertInStatusLine(App app)
    {
        var caption = Caption(app);
        var connection = Drawn(app, "ConnectionStatus");
        var list = Drawn(app, "RowsList");
        var captionCentre = caption.Y + (caption.Height / 2);
        var connectionCentre = connection.Y + (connection.Height / 2);
        Assert.True(Math.Abs(captionCentre - connectionCentre) <= 4 * Input.Scale(caption.Window),
            $"the caption's centre ({captionCentre}) lines up with the connection status' ({connectionCentre}): they share the footer's status line");
        Assert.True(caption.Y >= list.Y + list.Height,
            $"the caption's top ({caption.Y}) is at or below the list's bottom ({list.Y + list.Height})");
    }
}
