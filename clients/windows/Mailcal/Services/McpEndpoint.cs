// Where the local MCP server listens on this PC, and the relay an MCP client starts to reach it
// (docs/mcp.md). The core builds the configuration snippet from both. The Windows twin of the
// Apple client's McpEndpoint.
//
// WinUI-free and WinRT-free on purpose, so Mailcal.Tests can link it: every value here has to
// match something outside this file, the pipe name the core binds and the alias the app manifest
// registers, and each of those fails silently. The one thing that genuinely needs WinRT is "am I
// packaged", so that arrives as a parameter, exactly as OAuthScheme.For(bool) does.

using System.IO;

namespace Allodia.Mailcal.Services;

/// <summary>The local MCP server's endpoint and its relay command.</summary>
internal static class McpEndpoint
{
    /// <summary>The relay executable's file name, built from <c>crates/mailcal-mcp-shim</c> and
    /// laid down beside <c>Mailcal.exe</c>.</summary>
    public const string RelayFileName = "mailcal-mcp.exe";

    /// <summary>The App Execution Alias a packaged build registers for the relay, which is the
    /// command a person configures: the brand's <c>MAILCAL_MCP_RELAY_NAME</c>, which
    /// <c>scripts/dev/msix_manifest.py</c> writes into the package's manifest.</summary>
    public const string RelayAlias = Brand.McpRelayName + ".exe";

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
            return RelayAlias;
        }
        var beside = Path.Combine(appDirectory, RelayFileName);
        return File.Exists(beside) ? beside : RelayAlias;
    }
}
