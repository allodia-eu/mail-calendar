// Every launch the client hands to the OS goes through ExternalLauncher, the one place a debug
// launch can hold it back (MAILCAL_NO_EXTERNAL_OPEN). A call to the OS launcher anywhere else still
// compiles and still works, and its only symptom is a browser tab left open by a run of the UI
// suites, so the client's sources are read for one.
using System.IO;
using System.Linq;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ExternalLauncherTests
{
    [Fact]
    public void OnlyExternalLauncherHandsALaunchToTheOs()
    {
        var client = Path.Combine(Repo(), "clients", "windows", "Mailcal");
        var sources = Directory.EnumerateFiles(client, "*.cs", SearchOption.AllDirectories)
            .Where(path => !path.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}")
                && !path.Contains($"{Path.DirectorySeparatorChar}bin{Path.DirectorySeparatorChar}"))
            .ToList();
        Assert.Contains(sources, path => Path.GetFileName(path) == "ExternalLauncher.cs");
        var elsewhere = sources
            .Where(path => Path.GetFileName(path) != "ExternalLauncher.cs" && File.ReadAllText(path).Contains("LaunchUriAsync"))
            .Select(path => Path.GetRelativePath(client, path))
            .ToList();
        Assert.True(elsewhere.Count == 0, $"these launch through the OS directly, past ExternalLauncher: {string.Join(", ", elsewhere)}");
    }

    private static string Repo()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "VERSION")) && Directory.Exists(Path.Combine(dir.FullName, "clients", "windows")))
            {
                return dir.FullName;
            }
        }
        throw new InvalidOperationException($"no repository above {AppContext.BaseDirectory}");
    }
}
