// What the window shows while the core's constructor runs (docs/boot-sequence.md, "Hosting the
// constructor: the launch view"). The delay is reported to the state rather than slept through.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class LaunchStateTests
{
    // Inside the delay the window is a blank page: opening, with nothing said.
    [Fact]
    public void BeforeTheDelayNothingShows()
    {
        var launch = new LaunchState();
        launch.Begin();

        Assert.True(launch.Opening);
        Assert.False(launch.ShowsStatus);
    }

    [Fact]
    public void AfterTheDelayTheStatusShows()
    {
        var launch = new LaunchState();
        launch.Begin();

        launch.DelayElapsed();

        Assert.True(launch.Opening);
        Assert.True(launch.ShowsStatus);
    }

    // The normal open: the constructor returns inside the delay, and a delay that runs out after
    // that (its continuation was already queued) does not bring the status up over the app.
    [Fact]
    public void AConstructorReturningInsideTheDelayNeverShowsTheStatus()
    {
        var launch = new LaunchState();
        launch.Begin();

        launch.Opened();
        Assert.False(launch.Opening);
        Assert.False(launch.ShowsStatus);

        launch.DelayElapsed();
        Assert.False(launch.ShowsStatus);
    }

    [Fact]
    public void AConstructorReturningAfterTheDelayTakesTheStatusDown()
    {
        var launch = new LaunchState();
        launch.Begin();
        launch.DelayElapsed();

        launch.Opened();

        Assert.False(launch.Opening);
        Assert.False(launch.ShowsStatus);
    }
}
