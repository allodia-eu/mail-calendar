// An avatar for a fixture that does not draw one.
//
// The letters and the colour are the core's to derive, and every suite that needs one here is about
// something else, the auto-advance, a merged row, which row the list highlights. One stub keeps
// that decision out of them. The Android twin is AvatarFixture.kt.

using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Tests;

internal static class AvatarFixture
{
    /// <summary>The core's avatar record, in the shape the FFI hands one over.</summary>
    internal static Avatar Core(string initials = "SR") =>
        new(Initials: initials,
            Light: new(Background: "#4C6EF5", Text: "#FFFFFF", Border: "#3B5BDB"),
            Dark: new(Background: "#4C6EF5", Text: "#FFFFFF", Border: "#3B5BDB"),
            ImagePath: null);

    /// <summary>The same avatar, projected into what the views bind.</summary>
    internal static AvatarItem Item(string initials = "SR") => AvatarItem.From(Core(initials));
}
