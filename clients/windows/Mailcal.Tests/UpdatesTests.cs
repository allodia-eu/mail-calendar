// Which mechanism Settings → About says keeps this copy current.
//
// Windows ships this app twice (docs/windows-channels.md) and the two are updated by different
// things. Getting this wrong is not a visible bug: the page still draws, the button still works,
// and it queries a mechanism that does not hold this copy's updates. A Store user would be told
// there is nothing newer, forever, by a check that was asking the wrong service.
//
// Every case is decided by `Package.Current`, which throws or answers differently in each shape and
// cannot be exercised from a plain net10.0 suite. So the resolution is a pure function over the two
// facts, and this is where they are pinned.

using System;
using Allodia.Mailcal.Services;
using Xunit;

namespace Mailcal.Tests;

public class UpdatesTests
{
    [Fact]
    public void TheDevLoopHasNothingToUpdate()
    {
        // Unpackaged: there is no package to replace. The group is absent rather than showing a
        // button that could only ever fail.
        Assert.Equal(UpdateChannel.None, Updates.ChannelFor(packaged: false, updateSource: null));
    }

    [Fact]
    public void AnUnpackagedBuildStaysNoneEvenGivenAUrl()
    {
        // Defensive rather than reachable: packaging is what decides, and a URL from anywhere else
        // must not promote a build that has no package identity.
        Assert.Equal(
            UpdateChannel.None,
            Updates.ChannelFor(packaged: false, updateSource: new Uri("https://d/x.appinstaller")));
    }

    [Fact]
    public void APackageWithNoInstallerFileBelongsToTheStore()
    {
        // The Store's own packages carry no `.appinstaller`, so this is how a Store install is
        // told apart from ours without reading the brand.
        Assert.Equal(UpdateChannel.Store, Updates.ChannelFor(packaged: true, updateSource: null));
    }

    [Fact]
    public void APackageInstalledFromAnAppInstallerUpdatesFromIt()
    {
        Assert.Equal(
            UpdateChannel.Hosted,
            Updates.ChannelFor(packaged: true, updateSource: new Uri("https://d/x.appinstaller")));
    }
}
