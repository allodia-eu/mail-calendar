// What the reading pane shows between two messages.
//
// The reported bug is `OpeningAnotherMessageHoldsTheRenderedOne`: clicking a second message tore
// the pane down to a spinner for the ~200 ms the body took to arrive, so the recipient rows and
// the remote-images bar collapsed and came back and the message canvas blinked to the pane's own
// background in between, a visible flash on every click. Nothing that renders a frame can catch
// it: a screenshot lands on one side of it or the other, and a UI-automation assertion waits for
// the UI to settle, which is exactly the state the flash is not in.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ReadingHandoverTests
{
    // Nothing has ever been rendered, the first message of a session goes straight to the
    // spinner, because there is nothing on screen for it to stand behind.
    [Fact]
    public void TheFirstMessageOfASessionShowsTheLoadingState()
    {
        var handover = new ReadingHandover();
        Assert.Equal(HandoverStep.Loading, handover.Next("a"));
    }

    // The reported bug: with a message on screen, opening another one keeps it there while the new
    // body is fetched. The grace window is armed once, then every later re-render holds.
    [Fact]
    public void OpeningAnotherMessageHoldsTheRenderedOne()
    {
        var handover = new ReadingHandover();
        handover.Rendered("a");

        Assert.Equal(HandoverStep.StartGrace, handover.Next("b"));
        // Opening a message raises two changes (the header, then the cleared body), and a snapshot
        // refresh can raise more; only the first arms the timer.
        Assert.Equal(HandoverStep.Hold, handover.Next("b"));
        Assert.Equal(HandoverStep.Hold, handover.Next("b"));
    }

    // The hold is bounded: a fetch slower than the grace window gets its spinner, so a stale
    // message never stands in front of one the user is actually waiting for.
    [Fact]
    public void ASlowFetchFallsBackToTheLoadingState()
    {
        var handover = new ReadingHandover();
        handover.Rendered("a");
        Assert.Equal(HandoverStep.StartGrace, handover.Next("b"));

        handover.GraceElapsed();

        Assert.Equal(HandoverStep.Loading, handover.Next("b"));
        // And it stays there, a spent window is not re-armed by the next re-render, which would
        // put the pane back on the old message after it had already left it.
        Assert.Equal(HandoverStep.Loading, handover.Next("b"));
    }

    // Once the pane has dropped to the spinner there is nothing rendered to hold, so a third
    // message opened during that wait does not resurrect the second one's stand-in.
    [Fact]
    public void AMessageOpenedWhileLoadingDoesNotReviveTheStandIn()
    {
        var handover = new ReadingHandover();
        handover.Rendered("a");
        handover.Next("b");
        handover.GraceElapsed();
        Assert.Equal(HandoverStep.Loading, handover.Next("b"));
        handover.Cleared(); // the view has drawn the spinner

        Assert.Equal(HandoverStep.Loading, handover.Next("c"));
    }

    // Retrying a message whose body failed to fetch: the view reports the error panel as nothing
    // rendered (there is no message on it to hold), so pressing Retry answers with the spinner at
    // once instead of sitting on the error for the length of a grace window.
    [Fact]
    public void RetryingAFailedFetchShowsTheLoadingStateAtOnce()
    {
        var handover = new ReadingHandover();
        handover.Cleared(); // the view drew the "couldn't load, retry" panel

        Assert.Equal(HandoverStep.Loading, handover.Next("a"));
    }

    // Clicking a third message while the second is still being fetched re-arms the window around
    // the message actually on screen, rather than letting the first click's window expire under it.
    [Fact]
    public void AThirdClickDuringTheHoldRearmsTheWindow()
    {
        var handover = new ReadingHandover();
        handover.Rendered("a");
        Assert.Equal(HandoverStep.StartGrace, handover.Next("b"));
        Assert.Equal(HandoverStep.Hold, handover.Next("b"));

        Assert.Equal(HandoverStep.StartGrace, handover.Next("c"));
        Assert.Equal(HandoverStep.Hold, handover.Next("c"));
    }

    // The body landed: the pane is rendering the new message, so the next one is held against it
    // in turn.
    [Fact]
    public void TheBodyArrivingMakesTheNewMessageTheOneHeldAgainst()
    {
        var handover = new ReadingHandover();
        handover.Rendered("a");
        handover.Next("b");
        handover.Rendered("b");

        Assert.Equal(HandoverStep.StartGrace, handover.Next("c"));
    }
}
