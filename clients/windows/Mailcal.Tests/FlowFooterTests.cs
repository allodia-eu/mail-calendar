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

    [Fact]
    public void AStageComesInFromTheSideTheFlowMovedTowards()
    {
        var first = new FlowSteps(3, 0, "1 of 3");
        var second = new FlowSteps(3, 1, "2 of 3");
        Assert.Equal(FlowArrival.FromEnd, FlowFooter.Arrival(first, second));
        Assert.Equal(FlowArrival.FromStart, FlowFooter.Arrival(second, first));
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(second, second));
    }

    // A flow that gives no count says nothing about which way it moved.
    [Fact]
    public void WithoutACountAStageComesInInPlace()
    {
        var counted = new FlowSteps(3, 1, "2 of 3");
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(null, counted));
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(counted, null));
        Assert.Equal(FlowArrival.InPlace, FlowFooter.Arrival(null, null));
    }
}
