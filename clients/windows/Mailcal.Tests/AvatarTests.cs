// What the circle beside a person draws, and when two of them are the same avatar.
//
// Each rule here fails silently in the app rather than loudly. The equality one is the sharpest:
// a photo arrives in a LATER snapshot than the row it belongs to (docs/avatars.md, "Resolution
// never blocks a row"), so if two avatars differing only by their photo compared equal, the
// projection's reconcile would keep the row it already had and no face would ever appear, a
// feature that never runs, behind rows that look perfectly correct.

using Allodia.Mailcal.ViewModels;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AvatarTests
{
    private static AvatarItem WithPhoto(string path) => AvatarFixture.Item() with { ImagePath = path };

    [Fact]
    public void TheCoresLettersAndBothThemesColoursSurviveTheProjection()
    {
        var avatar = AvatarItem.From(new uniffi.mailcal_bindings.Avatar(
            Initials: "AL",
            Light: new(Background: "#2f6fa8", Text: "#FFFFFF", Border: "#25587f"),
            Dark: new(Background: "#7fb2dd", Text: "#000000", Border: "#5f95c4"),
            ImagePath: "C:/blobs/face.jpg"));

        Assert.Equal("AL", avatar.Initials);
        Assert.Equal("#2f6fa8", avatar.Background(dark: false));
        Assert.Equal("#FFFFFF", avatar.TextColor(dark: false));
        Assert.Equal("#7fb2dd", avatar.Background(dark: true));
        Assert.Equal("#000000", avatar.TextColor(dark: true));
        Assert.Equal("C:/blobs/face.jpg", avatar.ImagePath);
    }

    // The preference order the contract binds every platform to: photo, then monogram, never blank.
    [Fact]
    public void APhotoIsDrawnInPlaceOfTheLetters()
    {
        Assert.Equal(AvatarContent.Photo, WithPhoto("C:/blobs/face.jpg").Content);
    }

    [Fact]
    public void WithoutAPhotoTheLettersAreDrawn()
    {
        Assert.Equal(AvatarContent.Monogram, AvatarFixture.Item("AL").Content);
    }

    // Neither a name nor an address. The core sends no placeholder text, any word it chose would
    // be untranslatable English, so the client draws its own person glyph rather than an empty
    // circle. "Never blank" is the whole rule.
    [Fact]
    public void ARowThatNamesNobodyFallsToThePlatformsPersonGlyph()
    {
        Assert.Equal(AvatarContent.PersonGlyph, AvatarFixture.Item(string.Empty).Content);
    }

    [Fact]
    public void AnEmptyPhotoPathIsNoPhoto()
    {
        // The FFI's option arrives as null, but an empty string would reach the file reader as a
        // path and fail there instead, after the letters had already been given up for it.
        Assert.Equal(AvatarContent.Monogram, WithPhoto(string.Empty).Content);
    }

    // THE RECONCILE RULE. Same person, same letters, same colour, and a face that has since been
    // resolved. These are not the same avatar, or the row never gets the picture.
    [Fact]
    public void APhotoArrivingMakesItADifferentAvatar()
    {
        Assert.NotEqual(AvatarFixture.Item(), WithPhoto("C:/blobs/face.jpg"));
    }

    [Fact]
    public void TheSameAvatarTwiceComparesEqual()
    {
        Assert.Equal(AvatarFixture.Item("AL"), AvatarFixture.Item("AL"));
        Assert.Equal(WithPhoto("C:/blobs/face.jpg"), WithPhoto("C:/blobs/face.jpg"));
    }
}
