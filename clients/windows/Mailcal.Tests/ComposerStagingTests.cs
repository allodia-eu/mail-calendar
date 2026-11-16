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

        Assert.Equal(directory, ComposerStaging.ComposerDirectoryOf(_root, directory));
        Assert.Equal(directory, ComposerStaging.ComposerDirectoryOf(_root + Path.DirectorySeparatorChar, directory));
        Assert.Equal(directory, ComposerStaging.ComposerDirectoryOf(_root, directory + Path.DirectorySeparatorChar));
    }

    [Fact]
    public void ADirectoryAnywhereElseNamesNothingToDelete()
    {
        // A share's copies sit beside the root under their own retention.
        var share = Path.Combine(Path.GetDirectoryName(_root)!, "shared-attachments");
        var notAComposer = Composer("keep");
        var nested = Path.Combine(Composer(), "inner");

        Assert.Null(ComposerStaging.ComposerDirectoryOf(_root, share));
        Assert.Null(ComposerStaging.ComposerDirectoryOf(_root, notAComposer));
        Assert.Null(ComposerStaging.ComposerDirectoryOf(_root, nested));
        Assert.Null(ComposerStaging.ComposerDirectoryOf(_root, _root));
        Assert.Null(ComposerStaging.ComposerDirectoryOf(_root, null));
        Assert.Null(ComposerStaging.ComposerDirectoryOf(_root, string.Empty));
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
