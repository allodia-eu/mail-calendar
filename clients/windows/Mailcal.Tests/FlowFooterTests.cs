// The flow host's footer (docs/ai.md, "The flow host"): which controls it draws, in which order,
// and which side a stage comes in from. Which stage is next and which control is held are the
// core's; the host only draws them.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class FlowFooterTests
{
    private static readonly ControlState Free = new(true, false);

    private static FlowAction Action(string label) => new(label, Free, () => { });

    [Fact]
    public void TheFooterIsCloseThenBackThenThePrimary()
    {
        var frame = new FlowFrame("t", Action("Cancel"), Back: Action("Back"), Primary: Action("Learn"));
        Assert.Equal(
            [(FlowRole.Close, "Cancel"), (FlowRole.Back, "Back"), (FlowRole.Primary, "Learn")],
            FlowFooter.Controls(frame).Select(control => (control.Role, control.Action.Label)));
    }

    [Fact]
    public void AStageWithoutBackOrAPrimaryDrawsOnlyClose() =>
        Assert.Equal([FlowRole.Close], FlowFooter.Controls(new FlowFrame("t", Action("Close"))).Select(control => control.Role));

    private static FlowFrame Frame(FlowSteps? steps = null, int? depth = null) => new("t", Action("Close"), Steps: steps, Depth: depth);

    [Fact]
    public void AStageComesInFromTheSideTheFlowMovedTowards()
    {
        var first = Frame(new FlowSteps(3, 0, "1 of 3"));
        var second = Frame(new FlowSteps(3, 1, "2 of 3"));
        Assert.Equal(FlowArrival.FromEnd, FlowFooter.Arrival(first, second));
        Assert.Equal(FlowArrival.FromStart, FlowFooter.Arrival(second, first));
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(second, second));
    }

    // A flow with no count says which way it moved by the stages behind each.
    [Fact]
    public void WithoutACountTheStagesBehindSayTheSide()
    {
        Assert.Equal(FlowArrival.FromEnd, FlowFooter.Arrival(Frame(depth: 1), Frame(depth: 2)));
        Assert.Equal(FlowArrival.FromStart, FlowFooter.Arrival(Frame(depth: 2), Frame(depth: 1)));
    }

    // Nothing that says which way it moved, nothing that moves.
    [Fact]
    public void WithNeitherACountNorADepthAStageComesInInPlace()
    {
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(null, Frame(new FlowSteps(3, 1, "2 of 3"))));
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(Frame(), Frame()));
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(Frame(new FlowSteps(3, 1, "2 of 3")), Frame(depth: 2)));
    }
}
