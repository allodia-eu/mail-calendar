// Where the local MCP server listens on this PC, and the configuration snippet an MCP client needs
// to reach it (docs/mcp.md). The Windows twin of the Apple client's McpEndpoint.
//
// WinUI-free and WinRT-free on purpose, so Mailcal.Tests can link it: every value here has to
// match something outside this file, the pipe name the core binds, the alias the app manifest
// registers, the config key the protocol advertises, and each of those fails silently. The one
// thing that genuinely needs WinRT is "am I packaged", so that arrives as a parameter, exactly as
// OAuthScheme.For(bool) does.

using System.IO;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Allodia.Mailcal.Services;

/// <summary>The local MCP server's endpoint, its relay command, and the client config snippet.</summary>
internal static class McpEndpoint
{
    /// <summary>
    /// The key the generated snippet files this server under, <b>letters, numbers, hyphens and
    /// underscores only</b>, and identical to the <c>name</c> the protocol advertises
    /// (<c>crates/mailcal-mcp/src/branding.rs</c>). One identifier, not two.
    /// </summary>
    /// <remarks>
    /// It is tempting to use the display name here, because Claude Desktop labels a locally
    /// configured server by its config key and "Allodia Mail &amp; Calendar" renders beautifully
    /// there. It is also a trap: Claude Code accepts only <c>[A-Za-z0-9_-]</c> in a server name and
    /// <b>skips a Claude Desktop server whose name contains a space when importing it</b>, while a
    /// name embedded in a tool identifier has every other character rewritten to <c>_</c>. A pretty
    /// key looks right in one client and quietly breaks the next. The display name is
    /// <c>serverInfo.title</c>'s job.
    /// </remarks>
    public const string ConfigurationKey = "allodia-mail-and-calendar";

    /// <summary>The relay executable's file name, built from <c>crates/mailcal-mcp-shim</c> and
    /// laid down beside <c>Mailcal.exe</c>.</summary>
    public const string RelayFileName = "allodia-mcp.exe";

    /// <summary>
    /// The pipe the core listens on: <c>\\.\pipe\&lt;scheme&gt;.mcp</c>, where <c>&lt;scheme&gt;</c>
    /// is this build's OAuth redirect scheme.
    /// </summary>
    /// <remarks>
    /// Riding on <see cref="OAuthScheme"/> rather than minting a second discriminator is the point:
    /// a developer's machine has the Store build installed beside the dev one, and they must not
    /// share an endpoint, whichever started first would silently own the other's clients, and
    /// <c>first_pipe_instance</c> would refuse the second app's listener for reasons that look
    /// nothing like "two builds, one name". One packaging predicate decides both.
    /// </remarks>
    public static string PipeName(bool packaged) => $@"\\.\pipe\{OAuthScheme.For(packaged)}.mcp";

    /// <summary>
    /// The command an MCP client should spawn to reach this build, given the directory the app
    /// runs from.
    /// </summary>
    /// <remarks>
    /// The two shapes differ, and the packaged one is not a preference. A packaged app installs
    /// under <c>C:\Program Files\WindowsApps\…</c>, whose ACLs deny execution to an ordinary
    /// process, an absolute path in there would produce a config that looks right and fails with
    /// an access denial the user cannot act on. The App Execution Alias
    /// (<c>Package.appxmanifest</c>) exists for exactly this: it puts a launcher in
    /// <c>%LOCALAPPDATA%\Microsoft\WindowsApps</c>, which is on the user's PATH, so the bare name
    /// resolves. The unpackaged dev build has no alias and is reached by its absolute path.
    /// <para>
    /// A missing relay falls back to the bare name rather than emitting a path to nothing: if it is
    /// on PATH the snippet still works, and if it is not, "command not found" is a better message
    /// than a full path that does not exist.
    /// </para>
    /// </remarks>
    public static string RelayCommand(bool packaged, string appDirectory)
    {
        if (packaged)
        {
            return RelayFileName;
        }
        var beside = Path.Combine(appDirectory, RelayFileName);
        return File.Exists(beside) ? beside : RelayFileName;
    }

    /// <summary>
    /// The MCP-client configuration snippet for <paramref name="endpoint"/>, ready to paste.
    /// </summary>
    /// <remarks>
    /// Serialized, never concatenated. On Windows this matters more than anywhere else: the relay's
    /// path is full of backslashes and the pipe name <i>begins</i> with two, and a hand-built string
    /// would emit them raw, producing a config file that looks right, parses wrong, and whose
    /// failure (<c>\p</c> is not a valid JSON escape) names a line rather than a cause.
    /// </remarks>
    public static string ConfigurationSnippet(string endpoint, string relayCommand)
    {
        var config = new ClientConfig(new Dictionary<string, ServerEntry>
        {
            [ConfigurationKey] = new(relayCommand, new[] { "--endpoint", endpoint }),
        });
        return JsonSerializer.Serialize(config, SnippetOptions);
    }

    // Indented because a human reads this before pasting it. The relaxed encoder keeps a non-ASCII
    // user name legible (C:\Users\José, not C:\Users\Jos\u00E9), it relaxes HTML escaping, which a
    // JSON config file has no use for, and does NOT touch the backslash escaping JSON requires.
    private static readonly JsonSerializerOptions SnippetOptions = new()
    {
        WriteIndented = true,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private sealed record ClientConfig(
        [property: JsonPropertyName("mcpServers")] Dictionary<string, ServerEntry> McpServers);

    private sealed record ServerEntry(
        [property: JsonPropertyName("command")] string Command,
        [property: JsonPropertyName("args")] string[] Args);
}
