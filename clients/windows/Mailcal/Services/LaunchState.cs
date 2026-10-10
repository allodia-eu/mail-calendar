// What the main window shows while the core's constructor runs (docs/boot-sequence.md, "Hosting
// the constructor: the launch view"): a blank page until the core's launch-status delay has
// passed, then the opening status, and neither once the constructor has returned.
//
// Pure BCL on purpose: Mailcal.Tests links this file. The delay and the bindings are
// MailboxModel.Launch.cs.

namespace Allodia.Mailcal.Services;

/// <summary>Whether the window is waiting on the core's constructor, and whether it says so yet.</summary>
public sealed class LaunchState
{
    private bool _delayElapsed;

    /// <summary>The constructor is running, so no other top-level surface may show.</summary>
    public bool Opening { get; private set; }

    /// <summary>Show the progress indicator and "Opening your mailbox…".</summary>
    public bool ShowsStatus => Opening && _delayElapsed;

    /// <summary>The constructor has been called.</summary>
    public void Begin()
    {
        Opening = true;
        _delayElapsed = false;
    }

    /// <summary>The launch-status delay ran out. It changes nothing once the constructor has returned.</summary>
    public void DelayElapsed() => _delayElapsed = true;

    /// <summary>The constructor returned, with an app or with an error.</summary>
    public void Opened() => Opening = false;
}
