// The agent (MCP) endpoint's Windows shape (docs/mcp.md): the pipe name each build claims, the
// relay command an MCP client is told to spawn, and the config snippet it is told to paste.
//
// Every one of these fails SILENTLY in the app. A pipe name that drifts from the OAuth scheme means
// the dev build and the Store build fight over one endpoint, and the loser's server simply never
// listens. A snippet built by string concatenation emits a Windows path's backslashes raw, so the
// config file looks right and the client reports a parse error on a line rather than a cause. And a
// config key with a character some client rewrites is a server that works in one assistant and is
// skipped by the next. So each is pinned here rather than trusted.

using System.IO;
using System.Linq;
using System.Text.Json;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class McpEndpointTests
{
    [Fact]
    public void The_pipe_name_is_a_valid_windows_pipe_under_this_builds_oauth_scheme()
    {
        // `\\.\pipe\…` is what Endpoint::parse accepts as a pipe (crates/mailcal-mcp/src/endpoint.rs);
        // anything else it treats as a filesystem path and the server never binds.
        Assert.Equal($@"\\.\pipe\{OAuthScheme.Packaged}.mcp", McpEndpoint.PipeName(packaged: true));
        Assert.Equal($@"\\.\pipe\{OAuthScheme.Unpackaged}.mcp", McpEndpoint.PipeName(packaged: false));
    }

    [Fact]
    public void A_dev_build_and_a_store_build_do_not_collide()
    {
        // Both are installed on a developer's machine constantly. Sharing one endpoint would mean
        // whichever started first silently owns the other's clients, and, because the listener
        // asks for `first_pipe_instance`, the second app's server would refuse to start for a
        // reason whose log line says nothing about there being two builds.
        Assert.NotEqual(McpEndpoint.PipeName(packaged: true), McpEndpoint.PipeName(packaged: false));
        Assert.StartsWith(OAuthScheme.Packaged, McpEndpoint.PipeName(packaged: true).Split('\\')[^1], StringComparison.Ordinal);
        Assert.StartsWith(OAuthScheme.Unpackaged, McpEndpoint.PipeName(packaged: false).Split('\\')[^1], StringComparison.Ordinal);
    }

    [Fact]
    public void The_snippet_is_valid_json_with_a_windows_path_and_a_pipe_name()
    {
        // THE finding this test encodes: both values are backslash-heavy, and a hand-built JSON
        // string would emit them raw. `\p` is not a valid JSON escape, so the file the user pasted
        // fails to parse, after they have restarted their assistant and are looking at it.
        var relay = @"C:\Program Files\Some App With Spaces\allodia-mcp.exe";
        var endpoint = McpEndpoint.PipeName(packaged: true);

        var snippet = McpEndpoint.ConfigurationSnippet(endpoint, relay);

        using var parsed = JsonDocument.Parse(snippet);
        var entry = parsed.RootElement
            .GetProperty("mcpServers")
            .GetProperty(McpEndpoint.ConfigurationKey);
        Assert.Equal(relay, entry.GetProperty("command").GetString());
        var args = entry.GetProperty("args").EnumerateArray().Select(a => a.GetString()).ToArray();
        Assert.Equal(new[] { "--endpoint", endpoint }, args);
    }

    [Fact]
    public void The_snippet_names_exactly_one_server_under_a_portable_key()
    {
        // Letters, numbers, hyphens and underscores only. Claude Code accepts only [A-Za-z0-9_-] in
        // a server name and SKIPS a Claude Desktop server whose name contains a space when
        // importing it; where the name is embedded in a tool identifier, every other character is
        // rewritten to `_`. A pretty key looks right in one client and quietly breaks the next.
        Assert.All(
            McpEndpoint.ConfigurationKey,
            character => Assert.True(
                char.IsAsciiLetterLower(character) || char.IsAsciiDigit(character)
                    || character is '-' or '_',
                $"{McpEndpoint.ConfigurationKey} is not portable, use [a-z0-9-_] only"));
        // And it is the same string the protocol advertises as `name`
        // (crates/mailcal-mcp/src/branding.rs), one identifier, not two.
        Assert.Equal("allodia-mail-and-calendar", McpEndpoint.ConfigurationKey);

        using var parsed = JsonDocument.Parse(
            McpEndpoint.ConfigurationSnippet(@"\\.\pipe\x.mcp", "allodia-mcp.exe"));
        var servers = parsed.RootElement.GetProperty("mcpServers").EnumerateObject().ToArray();
        Assert.Single(servers);
        Assert.Equal(McpEndpoint.ConfigurationKey, servers[0].Name);
    }

    [Fact]
    public void A_packaged_build_is_reached_by_its_alias_never_by_an_installed_path()
    {
        // A packaged app installs under C:\Program Files\WindowsApps\…, whose ACLs deny execution
        // to an ordinary process, so an absolute path there produces a config that looks right and
        // fails with an access denial the user cannot act on. The App Execution Alias
        // (Package.appxmanifest) puts a launcher on PATH instead, so the bare name resolves.
        var command = McpEndpoint.RelayCommand(packaged: true, appDirectory: @"C:\anything");
        Assert.Equal(McpEndpoint.RelayFileName, command);
        Assert.False(Path.IsPathRooted(command));
    }

    [Fact]
    public void An_unpackaged_build_points_at_the_relay_beside_the_exe()
    {
        var directory = Directory.CreateTempSubdirectory("mcp-relay-test");
        try
        {
            var relay = Path.Combine(directory.FullName, McpEndpoint.RelayFileName);
            File.WriteAllText(relay, string.Empty);
            Assert.Equal(relay, McpEndpoint.RelayCommand(packaged: false, directory.FullName));

            // With the relay missing, fall back to the bare name rather than emitting a path to
            // nothing: if it is on PATH the snippet still works, and if it is not, "command not
            // found" beats a full path that does not exist.
            File.Delete(relay);
            Assert.Equal(
                McpEndpoint.RelayFileName,
                McpEndpoint.RelayCommand(packaged: false, directory.FullName));
        }
        finally
        {
            directory.Delete(recursive: true);
        }
    }

    [Fact]
    public void The_app_manifest_registers_the_alias_the_packaged_snippet_names()
    {
        // The coupling that makes the packaged path work at all, and which nothing else checks: the
        // command in the generated snippet is only resolvable because Package.appxmanifest declares
        // an App Execution Alias of exactly that name.
        var manifest = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Package.appxmanifest"));
        Assert.Contains(
            $"<uap5:ExecutionAlias Alias=\"{McpEndpoint.RelayFileName}\" />",
            manifest,
            StringComparison.Ordinal);
    }
}
