// What a multi-step flow hands the flow host (docs/ai.md, "The flow host"): the title, the
// footer's controls each with the ControlState the core gave it, and a step count only when the
// record gives one. The host holds no step, index or rule of its own; the one thing it decides is
// which side a stage arrives from. WinUI-free, so Mailcal.Tests can link it; the host itself is
// Controls/FlowHost.cs.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>A control in the host's footer: its words, its state from the core and what a press
/// sends. A press arrives only while the state lets it.</summary>
internal sealed record FlowAction(string Label, ControlState State, Action Press);

/// <summary>
/// Where the flow stands, when its record says: <paramref name="Current"/> of
/// <paramref name="Count"/>, named <paramref name="Name"/> for assistive technology. A paged result
/// that may be read in any order passes <paramref name="Go"/>, which a pip calls with its page; a
/// staged flow passes none, and its pips only show.
/// </summary>
internal sealed record FlowSteps(int Count, int Current, string Name, Action<int>? Go = null);

/// <summary>One stage as the host draws it. <paramref name="Steps"/> is null for a flow with no
/// known number of steps, which draws no pips.</summary>
internal sealed record FlowFrame(
    string Title, FlowAction Close, FlowAction? Back = null, FlowAction? Primary = null, FlowSteps? Steps = null);

/// <summary>What a footer control is to the host.</summary>
internal enum FlowRole
{
    /// <summary>Cancel or Close, first in the footer.</summary>
    Close,

    /// <summary>Back, before the primary.</summary>
    Back,

    /// <summary>The primary action, last and in the accent style.</summary>
    Primary,
}

/// <summary>Which side a stage comes in from.</summary>
internal enum FlowArrival
{
    /// <summary>Without moving: the same step, or a flow that gives no count.</summary>
    InPlace,

    /// <summary>From the end, as a later step does.</summary>
    FromEnd,

    /// <summary>From the start, as an earlier step does.</summary>
    FromStart,
}

internal static class FlowFooter
{
    /// <summary>The footer's controls in the order they are drawn: Close, then Back, then the
    /// primary, each only where the frame has it.</summary>
    internal static IReadOnlyList<(FlowRole Role, FlowAction Action)> Controls(FlowFrame frame)
    {
        var controls = new List<(FlowRole, FlowAction)> { (FlowRole.Close, frame.Close) };
        if (frame.Back is { } back)
        {
            controls.Add((FlowRole.Back, back));
        }
        if (frame.Primary is { } primary)
        {
            controls.Add((FlowRole.Primary, primary));
        }
        return controls;
    }

    /// <summary>The side the stage <paramref name="to"/> comes in from after <paramref name="from"/>:
    /// only a count both give says which way the flow moved.</summary>
    internal static FlowArrival Arrival(FlowSteps? from, FlowSteps? to) => (from, to) switch
    {
        ({ } before, { } after) when after.Current > before.Current => FlowArrival.FromEnd,
        ({ } before, { } after) when after.Current < before.Current => FlowArrival.FromStart,
        _ => FlowArrival.InPlace,
    };
}
