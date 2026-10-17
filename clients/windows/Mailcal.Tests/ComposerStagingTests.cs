// Which directories of staged composer files are deleted (Services/ComposerStaging.cs).
//
// Every test works in a directory of its own under the host's temporary directory, so the rule is
// stated about paths it made rather than about this machine's app data.
using System;
using System.IO;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class ComposerStagingTests : IDisposable
{
    private readonly string _root =
        Path.Combine(Path.GetTempPath(), "mailcal-staging-tests-" + Guid.NewGuid().ToString("N"));

    public void Dispose()
    {
        if (Directory.Exists(_root))
        {
            Directory.Delete(_root, recursive: true);
        }
    }

    private string Composer(string? name = null)
    {
        var directory = Path.Combine(_root, name ?? Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        File.WriteAllText(Path.Combine(directory, "0-report.csv"), "a,b");
        return directory;
    }

    [Fact]
    public void ADiscardedComposerNamesTheDirectoryItsFilesWereStagedIn()
    {
        var directory = Composer();
        var staged = new[] { Path.Combine(directory, "0-report.csv"), Path.Combine(directory, "1-plan.pdf") };

        Assert.Equal([directory], ComposerStaging.DirectoriesOf(_root, staged));
        Assert.Equal([directory], ComposerStaging.DirectoriesOf(_root + Path.DirectorySeparatorChar, staged));
    }

    [Fact]
    public void AFileStagedAnywhereElseNamesNothingToDelete()
    {
        // A share's copy sits beside the root under its own retention, and a composer can hold it.
        var share = Path.Combine(Path.GetDirectoryName(_root)!, "shared-attachments", "1-photo.jpg");
        var notAComposer = Path.Combine(Composer("keep"), "0-report.csv");
        var nested = Path.Combine(Composer(), "inner", "0-report.csv");

        Assert.Empty(ComposerStaging.DirectoriesOf(_root, [share, notAComposer, nested]));
    }

    [Fact]
    public void ALaunchDeletesWhatEarlierComposersStaged()
    {
        var earlier = Composer();

        Assert.Equal(1, ComposerStaging.SweepIn(_root, DateTime.UtcNow.AddMinutes(1)));
        Assert.False(Directory.Exists(earlier));
    }

    [Fact]
    public void ALaunchLeavesAComposerOpenedSinceItAlone()
    {
        var since = Composer();

        Assert.Equal(0, ComposerStaging.SweepIn(_root, DateTime.UtcNow.AddHours(-1)));
        Assert.True(File.Exists(Path.Combine(since, "0-report.csv")));
    }

    [Fact]
    public void ALaunchLeavesWhatIsNotAComposersDirectoryAlone()
    {
        // The earlier roots are generic names in the user's temporary directory, so anything else
        // in one may be another program's.
        var foreign = Composer("someone-elses");
        File.WriteAllText(Path.Combine(_root, "notes.txt"), "kept");

        ComposerStaging.SweepIn(_root, DateTime.UtcNow.AddMinutes(1), dropEmptyRoot: true);

        Assert.True(Directory.Exists(foreign));
        Assert.True(File.Exists(Path.Combine(_root, "notes.txt")));
    }

    [Fact]
    public void AnEarlierRootGoesOnceNothingIsLeftInIt()
    {
        Composer();

        ComposerStaging.SweepIn(_root, DateTime.UtcNow.AddMinutes(1), dropEmptyRoot: true);

        Assert.False(Directory.Exists(_root));
    }

    [Fact]
    public void ALaunchBeforeAnythingWasStagedDeletesNothing()
    {
        Assert.Equal(0, ComposerStaging.SweepIn(_root, DateTime.UtcNow.AddMinutes(1)));
    }
}
