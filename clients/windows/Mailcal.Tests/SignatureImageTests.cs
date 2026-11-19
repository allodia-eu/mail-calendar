// A picked picture as the signature editor inserts it (docs/signatures.md). Whether it may go in
// at all is the core's rule, pinned in mailcal-app's signature_rules_tests.rs; what is pinned here
// is the shape the editor's insertSignatureImage seam reads, which fails silently when wrong: the
// bundle drops anything that is not a `data:image/` URI, and a payload under another key inserts
// nothing.

using System.Text.Json;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SignatureImageTests
{
    [Fact]
    public void ThePictureBecomesADataUriOfTheTypeTheSystemReports() =>
        Assert.Equal("data:image/png;base64,AQID", SignatureImage.DataUrl([1, 2, 3], "image/png"));

    [Fact]
    public void ThePayloadCarriesTheUriAndTheAltTextUnderTheKeysTheEditorReads()
    {
        using var payload = JsonDocument.Parse(SignatureImage.Payload("data:image/gif;base64,AQID", "logo"));
        Assert.Equal("data:image/gif;base64,AQID", payload.RootElement.GetProperty("data_url").GetString());
        Assert.Equal("logo", payload.RootElement.GetProperty("alt_text").GetString());
    }

    // The alt text is a file's name, which anyone can choose: it rides as a JSON string the editor
    // assigns to node.alt, so markup in it stays text.
    [Fact]
    public void AnAltTextHoldingMarkupStaysAString()
    {
        using var payload = JsonDocument.Parse(SignatureImage.Payload("data:image/png;base64,AQID", "\"><img src=x>"));
        Assert.Equal("\"><img src=x>", payload.RootElement.GetProperty("alt_text").GetString());
    }
}
