// The agent (MCP) surface of the model: the settings a Settings panel renders, the four decisions it
// can change, and the wiring that makes the server exist at all (docs/mcp.md). Its own partial to
// keep MailboxModel.cs under the 500-line limit.
//
// Nothing is cached here, on purpose, the same discipline as MailboxModel.Signatures.cs. The
// snapshot carries `running`, which is whether a pipe is actually bound rather than what the switch
// says, and the two can disagree (another instance owning the name, a name that will not bind). A
// mirrored copy would report the state at the last signal, which for exactly that field is the one
// thing worth being current.

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

    /// <summary>
    /// The local MCP settings a Settings panel renders, the user's decisions plus whether a server
    /// is actually listening, or <c>null</c> before the core is up.
    /// </summary>
    /// <remarks>
    /// <c>endpoint</c> is <c>null</c> on a platform whose host set none, which is how a Settings
    /// screen knows not to offer the panel at all. Windows always sets one, so this is the "not
    /// connected yet" case rather than a platform check.
    /// </remarks>
    internal McpSettings? Mcp => _app?.McpSettings();

    /// <summary>Turns the local MCP server on or off. Off does <b>not</b> clear the exposed-account
    /// list, switching the feature off for an afternoon should not mean re-ticking every
    /// mailbox.</summary>
    internal void SetMcpEnabled(bool enabled) => _app?.SetMcpEnabled(enabled);

    /// <summary>Exposes or hides one account to assistants. Applied to a live connection at once,
    /// so unticking revokes access without a restart.</summary>
    internal void SetMcpAccountExposed(string account, bool exposed) =>
        _app?.SetMcpAccountExposed(account, exposed);

    /// <summary>Sets whether an assistant may send mail with no human review. With it off the send
    /// tool is <b>absent</b> from the server's listing entirely.</summary>
    internal void SetMcpAllowDirectSend(bool allow) => _app?.SetMcpAllowDirectSend(allow);

    /// <summary>Sets whether a direct send is restricted to people the user already emails, the
    /// guard that actually blocks exfiltration.</summary>
    internal void SetMcpRequireKnownRecipient(bool require) =>
        _app?.SetMcpRequireKnownRecipient(require);

    /// <summary>
    /// The MCP-client configuration snippet for this build, or <c>null</c> when there is no
    /// endpoint to point at. The very string the core listens on, never a second derivation.
    /// </summary>
    internal string? McpConfigurationSnippet() =>
        Mcp?.Endpoint is { } endpoint
            ? McpEndpoint.ConfigurationSnippet(
                endpoint,
                McpEndpoint.RelayCommand(AppIdentity.IsPackaged, AppContext.BaseDirectory))
            : null;

    /// <summary>
    /// Wires the agent surface onto a freshly-opened core: the composer port, then the endpoint.
    /// </summary>
    /// <remarks>
    /// Both calls are what make the server exist at all, without an endpoint the core has nowhere
    /// to listen, and without the composer port an assistant's <c>create_draft</c> reports that this
    /// build has no composer. Order matters only in that setting the endpoint applies the persisted
    /// settings and can start listening immediately, so the composer must already be installed when
    /// the first connection arrives.
    /// <para>
    /// A showcase run deliberately gets neither: its mailbox is fiction, and a screenshot build
    /// binding the real pipe would take the endpoint away from the real app running beside it.
    /// </para>
    /// </remarks>
    private void WireAgentAccess(MailcalApp app)
    {
        app.SetAgentHostUi(new AgentComposerBridge(_ui, draft => AgentDraftRequested?.Invoke(draft)));
        app.SetMcpEndpoint(McpEndpoint.PipeName(AppIdentity.IsPackaged));
    }
}
