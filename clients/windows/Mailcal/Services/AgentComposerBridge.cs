// The Windows half of the MCP `create_draft` tool: turning an assistant's draft into the app's own
// composer, prefilled and unsent (docs/mcp.md). The core hands the draft over the `AgentHostUi`
// port; this marshals it onto the UI thread, where the shell opens the composer the user knows.
//
// Deliberately NOT a send. The user sees the recipients and the body and presses Send themselves.
// That is a visibility property, not a safety guarantee, someone who asked for "reply to Bob" will
// press Send without reading, so it is not the control the design leans on (the known-recipient
// guard is). What it does buy is that an assistant's message appears in the user's own app, where
// it can be edited or abandoned.

using Microsoft.UI.Dispatching;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// The host side of the core's agent-UI port: an assistant's <c>create_draft</c> arrives here.
/// </summary>
/// <remarks>
/// The port's contract says an implementation <b>must not block</b>, this is invoked from the MCP
/// server's connection task, on a Rust runtime thread, and a host that waited for the window would
/// stall that connection and could deadlock against a single-threaded UI. So it enqueues and
/// returns: <see cref="DispatcherQueue.TryEnqueue(DispatcherQueueHandler)"/> is non-blocking, and
/// its <c>false</c> return (the queue is shutting down, i.e. the app is closing) is the right
/// no-op, there is no composer left to open into.
/// </remarks>
internal sealed class AgentComposerBridge : AgentHostUi
{
    private readonly DispatcherQueue _ui;
    private readonly Action<AgentDraft> _open;

    public AgentComposerBridge(DispatcherQueue ui, Action<AgentDraft> open)
    {
        _ui = ui;
        _open = open;
    }

    /// <inheritdoc/>
    public void OpenComposer(AgentDraft draft) => _ui.TryEnqueue(() => _open(draft));
}
