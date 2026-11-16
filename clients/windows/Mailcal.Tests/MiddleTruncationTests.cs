// How a composer tile cuts a file name too long for it (Services/MiddleTruncation.cs).
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class MiddleTruncationTests
{
    [Fact]
    public void ANameThatFitsIsLeftWhole()
    {
        Assert.Equal("report.pdf", MiddleTruncation.Cut("report.pdf", 10));
        Assert.Equal("report.pdf", MiddleTruncation.Cut("report.pdf", 40));
    }

    [Fact]
    public void TheEndOfTheNameIsKeptSoScreenshotsStillDiffer()
    {
        // Twenty screenshots share their first twenty characters; what tells them apart is the end.
        const string shot = "Schermafbeelding 2026-06-14 om 10.21.33.png";
        var cut = MiddleTruncation.Cut(shot, 24);
        Assert.Equal(24 + 1, MiddleTruncation.Length(cut));
        Assert.StartsWith("Schermafbeel", cut);
        Assert.EndsWith("10.21.33.png", cut);
        Assert.Contains(MiddleTruncation.Ellipsis, cut);
    }

    [Fact]
    public void ALongExtensionIsKeptWhileThereIsRoomBeforeIt()
    {
        Assert.Equal("a….presentation", MiddleTruncation.Cut("archive.presentation", 14));
        Assert.Equal("ar….xlsx", MiddleTruncation.Cut("archive-of-everything.xlsx", 7));
    }

    [Fact]
    public void NoRoomAtAllLeavesTheEllipsis()
    {
        Assert.Equal("…", MiddleTruncation.Cut("report.pdf", 0));
    }

    [Fact]
    public void ACharacterMadeOfSeveralCodeUnitsIsNeverSplit()
    {
        // A flag is two code points and four UTF-16 units; it counts, and is kept, as one.
        const string name = "trip 🇳🇱 photos from the summer 🇳🇱.jpg";
        var cut = MiddleTruncation.Cut(name, 12);
        Assert.Equal(13, MiddleTruncation.Length(cut));
        Assert.EndsWith("🇳🇱.jpg", cut);
    }
}
