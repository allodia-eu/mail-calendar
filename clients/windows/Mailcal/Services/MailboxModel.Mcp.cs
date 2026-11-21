// The agent (MCP) surface of the model and Settings → Advanced's traffic with the core: the page the
// core decides, the changes it dispatches, and the wiring that makes the server exist at all
// (docs/mcp.md). Nothing is cached here: the page is read from memory, and the server's status in
// it moves on Settings signals of its own.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Raised when an assistant asks to open a prefilled draft (<c>create_draft</c>). Already
    /// marshalled onto the UI thread by <see cref="AgentComposerBridge"/>. The shell owns the
    /// composer, so it subscribes; the model never opens one itself.
    /// </summary>
    internal event Action<AgentDraft>? AgentDraftRequested;

    /// <summary>Settings → Advanced as the core decides it, or <c>null</c> before the core is up.
    /// Read from memory, so the UI thread may ask it on every Settings signal.</summary>
    internal AdvancedPage? Advanced => _app?.AdvancedPage();

    /// <summary>A change to what assistants may do, held by the core before this returns.</summary>
    internal void SetAssistantAccess(McpIntent intent) => _app?.Dispatch(new Intent.Mcp(intent));

    /// <summary>Resets the local database, once the person confirmed it. The page says it runs and
    /// what it came to.</summary>
    internal void ResetDatabase() => _app?.Dispatch(new Intent.ResetDatabase());

    /// <summary>
    /// Wires the agent surface onto a freshly-opened core: the composer port, then the endpoint
    /// and the relay an assistant starts to reach it.
    /// </summary>
    /// <remarks>
    /// Setting the host applies the persisted settings and can start listening at once, so the
    /// composer port is installed first: an assistant's <c>create_draft</c> without it reports that
    /// this build has no composer.
    /// <para>
    /// A showcase run gets neither: its mailbox is fiction, and a screenshot build binding the real
    /// pipe would take the endpoint away from the real app running beside it.
    /// </para>
    /// </remarks>
    private void WireAgentAccess(MailcalApp app)
    {
        app.SetAgentHostUi(new AgentComposerBridge(_ui, draft => AgentDraftRequested?.Invoke(draft)));
        app.SetMcpHost(new McpHost(
            McpEndpoint.PipeName(AppIdentity.IsPackaged),
            McpEndpoint.RelayCommand(AppIdentity.IsPackaged, AppContext.BaseDirectory),
            Array.Empty<string>()));
    }
}
