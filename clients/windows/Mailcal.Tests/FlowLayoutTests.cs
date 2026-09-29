// The composer's recipient field lays its pills and its input out in one flow (Controls/FlowPanel):
// the input fills what is left of its line, so a header row stays one line until the pills need
// more, and it takes a line of its own only when too little is left to type in.

using System.Linq;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class FlowLayoutTests
{
    [Fact]
    public void TheInputFillsTheRestOfTheLineAfterThePills()
    {
        var flow = FlowLayout.Arrange([(100, 24), (80, 24), (60, 32)], 400, 4, fillLast: 120);
        var input = flow.Slots[^1];
        Assert.Equal(0, input.Y);
        Assert.Equal(188, input.X);
        Assert.Equal(212, input.Width);
        Assert.Equal(32, flow.Height);
    }

    [Fact]
    public void TheInputTakesALineOfItsOwnWhenTooLittleIsLeft()
    {
        var flow = FlowLayout.Arrange([(200, 24), (120, 24), (60, 32)], 400, 4, fillLast: 120);
        var input = flow.Slots[^1];
        Assert.Equal(0, input.X);
        Assert.Equal(400, input.Width);
        Assert.Equal(28, input.Y);
        Assert.Equal(60, flow.Height);
    }

    [Fact]
    public void AnEmptyFieldIsTheInputAloneAcrossTheLine()
    {
        var flow = FlowLayout.Arrange([(60, 32)], 400, 4, fillLast: 120);
        Assert.Equal(new FlowSlot(0, 0, 400, 32), flow.Slots.Single());
    }

    // A pill is shorter than the input, and sits level with the text typed beside it.
    [Fact]
    public void AShorterChildIsCentredOnItsLine()
    {
        var flow = FlowLayout.Arrange([(100, 24), (60, 32)], 400, 4, fillLast: 120);
        Assert.Equal(4, flow.Slots[0].Y);
        Assert.Equal(0, flow.Slots[1].Y);
    }

    // Without a filling child, the chips of the reveal flow as before: each keeps its own width.
    [Fact]
    public void WithoutFillingEveryChildKeepsItsWidthAndWraps()
    {
        var flow = FlowLayout.Arrange([(150, 20), (150, 20), (150, 20)], 400, 8);
        Assert.Equal(new[] { 150.0, 150.0, 150.0 }, flow.Slots.Select(slot => slot.Width).ToArray());
        Assert.Equal(28, flow.Slots[2].Y);
        Assert.Equal(308, flow.Width);
    }
}
