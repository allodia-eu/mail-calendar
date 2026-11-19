// The agent (MCP) endpoint's Windows shape (docs/mcp.md): the pipe name each build claims and the
// relay command an MCP client is told to spawn. The snippet built from them is the core's, and its
// tests are in crates/mailcal-mcp.
//
// Both fail SILENTLY in the app. A pipe name that drifts from the OAuth scheme means the dev build
// and the Store build fight over one endpoint, and the loser's server simply never listens. A relay
// command naming a path inside WindowsApps is a configuration that looks right and is refused. So
// each is pinned here rather than trusted.

using System.IO;
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
    public void A_packaged_build_is_reached_by_its_alias_never_by_an_installed_path()
    {
        // A packaged app installs under C:\Program Files\WindowsApps\…, whose ACLs deny execution
        // to an ordinary process, so an absolute path there produces a config that looks right and
        // fails with an access denial the user cannot act on. The App Execution Alias
        // (Package.appxmanifest) puts a launcher on PATH instead, so the bare name resolves.
        var command = McpEndpoint.RelayCommand(packaged: true, appDirectory: @"C:\anything");
        Assert.Equal(McpEndpoint.RelayAlias, command);
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
                McpEndpoint.RelayAlias,
                McpEndpoint.RelayCommand(packaged: false, directory.FullName));
        }
        finally
        {
            directory.Delete(recursive: true);
        }
    }

    [Fact]
    public void The_app_manifest_points_the_alias_at_the_relay_the_build_lays_down()
    {
        // The coupling that makes the packaged path work at all: the alias launches the file the
        // csproj copies to the package root, under exactly that name. The alias itself is the
        // brand's and is written at packaging time; test_msix_manifest.py pins that rewrite.
        var manifest = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Package.appxmanifest"));
        Assert.Contains(
            $"Executable=\"{McpEndpoint.RelayFileName}\"",
            manifest,
            StringComparison.Ordinal);
    }
}
