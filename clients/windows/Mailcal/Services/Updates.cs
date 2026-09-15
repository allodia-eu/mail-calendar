// How this copy of the app is kept current, and what a check came back with.
//
// Windows ships this app twice (docs/windows-channels.md), and the two channels are updated by
// different things: the Store updates what the Store installed, and App Installer updates what an
// `.appinstaller` installed. Settings -> About has to say the right one, because an answer that
// names the wrong mechanism sends a user somewhere that will never have their update.
//
// WinUI-free on purpose, so the resolution is unit-tested in Mailcal.Tests (UpdatesTests). The one
// `Package.Current` read it depends on stays in AppIdentity, as every other one does.

using System;

namespace Allodia.Mailcal.Services;

/// <summary>What keeps this installation current.</summary>
internal enum UpdateChannel
{
    /// <summary>Nothing does: the unpackaged dev loop has no package to replace.</summary>
    None,

    /// <summary>App Installer, from the `.appinstaller` this copy was installed from.</summary>
    Hosted,

    /// <summary>The Microsoft Store, which owns what it installed.</summary>
    Store,
}

/// <summary>What a check found, in the three terms the About page draws.</summary>
internal enum UpdateOutcome
{
    UpToDate,
    Available,
    Failed,
}

internal static class Updates
{
    /// <summary>Which mechanism keeps this installation current.</summary>
    /// <remarks>
    /// The discriminator is the platform's own answer rather than the brand: a package installed
    /// from an `.appinstaller` has one, and every other packaged build has none. Reading the
    /// identity name instead would be a guess that is wrong for a hand-sideloaded build, and it is
    /// exactly the build a support question comes from.
    /// </remarks>
    internal static UpdateChannel ChannelFor(bool packaged, Uri? updateSource) =>
        !packaged ? UpdateChannel.None
        : updateSource is null ? UpdateChannel.Store
        : UpdateChannel.Hosted;
}
