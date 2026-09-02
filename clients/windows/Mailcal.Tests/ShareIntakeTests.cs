// The staged-filename rule (Services/ShareStaging.cs).
//
// This is NOT the attachment's name: that one comes back from the shared core, already normalised,
// and is what a recipient reads (docs/os-integration.md). This one only has to be creatable on
// this device, and it fails by throwing at File.Create, which would lose a shared file for a
// reason the user could do nothing about.
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ShareIntakeTests
{
    [Fact]
    public void ACharacterWindowsWillNotPutInAFileNameBecomesAnUnderscore()
    {
        Assert.Equal("q1_q2_q3_.txt", ShareStaging.StagedName("q1:q2*q3?.txt"));
        // A control character too: legal in the string a sharing app hands over, illegal in a
        // path. A space is neither, and survives.
        Assert.Equal("a_b.pdf", ShareStaging.StagedName("a\u0001b.pdf"));
        Assert.Equal("a b.pdf", ShareStaging.StagedName("a b.pdf"));
    }

    [Fact]
    public void ANameThatIsAPathKeepsNoSeparators()
    {
        // A separator surviving here would make File.Create write outside the staging directory,
        // or throw. The attachment's own name is the core's answer and is unaffected either way.
        Assert.DoesNotContain("/", ShareStaging.StagedName("../../etc/passwd"));
        Assert.DoesNotContain("\\", ShareStaging.StagedName("photos\\holiday.jpg"));
    }

    [Fact]
    public void AnEmptyOrAllStrippedNameStillNamesSomething()
    {
        foreach (var candidate in new[] { "", "...", "   ", "___" })
        {
            Assert.Equal("shared", ShareStaging.StagedName(candidate));
        }
    }

    [Fact]
    public void ALongNameIsCutSoThePathStaysCreatable()
    {
        // The staged path also carries the staging directory and a GUID prefix, and it is their
        // sum that meets MAX_PATH; a 400-character name on top of those would pass it.
        Assert.Equal(80, ShareStaging.StagedName(new string('a', 400)).Length);
    }
}
