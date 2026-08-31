// The circle beside a person, in the shape the XAML binds: the monogram, the colour it sits on in
// each theme, and the photo when a synced address book has one. docs/avatars.md is the contract,
// what the circle is OF, which letters, which colour, and that it is decoration a screen reader
// must not read.
//
// A projection of the core's `Avatar` for the reason every view model here is one: the generated
// UniFFI records are internal and carry Rust field names. **Pure BCL besides**, no WinUI type
// reaches this file, which is what lets Mailcal.Tests link it and MessageStop
// (Services/ReadingAdvance.cs) carry one. Turning a hex into a brush and a path into a bitmap is
// the WinUI half, in Controls/AvatarView.cs.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.ViewModels;

/// <summary>What the circle draws.</summary>
public enum AvatarContent
{
    /// <summary>A photo from a synced address book, drawn to the circle's edge.</summary>
    Photo,

    /// <summary>The person's initials on their colour, the common case.</summary>
    Monogram,

    /// <summary>
    /// Neither a name nor an address, so the platform's own person glyph. The core deliberately
    /// sends no placeholder text: any word it chose would be untranslatable English.
    /// </summary>
    PersonGlyph,
}

/// <summary>One person's avatar, ready to draw.</summary>
/// <remarks>
/// A record for its value equality, which the snapshot reconcile depends on: a photo arrives in a
/// <em>later</em> snapshot than the row it belongs to (docs/avatars.md, "Resolution never blocks a
/// row"), so a row comparison that ignored the avatar would keep the container it already had and
/// the face would never appear, a row that looks correct, from a feature that never runs.
/// </remarks>
public sealed record AvatarItem
{
    // Constructed only through From. Private for a reason the XAML compiler makes concrete: a type
    // that is a DependencyProperty's type gets an activator emitted for it in XamlTypeInfo.g.cs if
    // it is publicly constructible, and `new AvatarItem()` cannot satisfy the required members
    // below. Keeping the door shut is also the honest shape, the contract says an avatar is never
    // blank, so a blank one has to be something a caller chose rather than something it got by
    // forgetting a field.
    private AvatarItem()
    {
    }

    /// <summary>One or two letters, uppercased; empty when the row names nobody.</summary>
    public required string Initials { get; init; }

    /// <summary>The circle's fill in a light theme, <c>#rrggbb</c>.</summary>
    public required string LightBackground { get; init; }

    /// <summary>The letters' colour on that fill, the core has already made it legible.</summary>
    public required string LightText { get; init; }

    /// <summary>The circle's fill in a dark theme.</summary>
    public required string DarkBackground { get; init; }

    /// <summary>The letters' colour in a dark theme.</summary>
    public required string DarkText { get; init; }

    /// <summary>
    /// A raster image on disk to draw instead of the monogram, or null. It is named by a hash of
    /// its own contents, so a client may cache against the path indefinitely; the core has already
    /// sniffed its magic bytes and its size, so nothing here checks the format or trusts a media
    /// type.
    /// </summary>
    public string? ImagePath { get; init; }

    /// <summary>What to draw: the photo, else the letters, else the platform's person glyph.</summary>
    public AvatarContent Content =>
        !string.IsNullOrEmpty(ImagePath) ? AvatarContent.Photo
        : Initials.Length > 0 ? AvatarContent.Monogram
        : AvatarContent.PersonGlyph;

    /// <summary>The circle's fill in the theme the window is painted in.</summary>
    public string Background(bool dark) => dark ? DarkBackground : LightBackground;

    /// <summary>The letters' colour in that theme.</summary>
    public string TextColor(bool dark) => dark ? DarkText : LightText;

    /// <summary>Projects the core's avatar into the shape the views bind.</summary>
    internal static AvatarItem From(Avatar avatar) => new()
    {
        Initials = avatar.Initials,
        LightBackground = avatar.Light.Background,
        LightText = avatar.Light.Text,
        DarkBackground = avatar.Dark.Background,
        DarkText = avatar.Dark.Text,
        ImagePath = avatar.ImagePath,
    };
}
