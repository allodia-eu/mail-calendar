// A message with more attachments than fit must not push its own body off the screen.
//
// WHY HERE. The rule is a layout one: the attachment panel sits in an Auto row above the body's *
// row, so an unbounded list takes what it wants and starves the message. Nothing that cannot link
// WinUI can see a row height.
//
// WHY THE HARNESS. The showcase seeds no message with enough attachments to reach the cap, and one
// attachment cannot tell a capped list from an uncapped one. docker/stalwart/seed.sh appends
// 10-many-attachments.eml (twenty files) for this.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class AttachmentsTests(HarnessApp harness)
{
    private const string ManyAttachments = "Message with many attachments";

    // Both in the units XAML is written in; the agent reports physical pixels, so each is scaled at
    // assert time. The cap is ReadingView.xaml's plus a little for the ScrollViewer's own chrome:
    // the assertion is "bounded", not "exactly this".
    private const double CapDipWithSlack = 200;

    // What is left for the message beside a full attachment bar: a few lines of mail, not a sliver.
    private const double BodyFloorDip = 120;

    [HarnessFact]
    public void TwentyAttachmentsStopAtTheCapInsteadOfTakingThePane() => harness.Case(nameof(TwentyAttachmentsStopAtTheCapInsteadOfTakingThePane), app =>
    {
        app.OpenMessage(ManyAttachments);
        var scroll = app.Agent.Wait(Filter.ById("AttachmentScroll") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("no AttachmentScroll: either the body never arrived, or the attachment panel is not the ScrollViewer this rule depends on");
        var cap = CapDipWithSlack * Input.Scale(scroll.Window);
        Assert.True(scroll.Height <= cap,
            $"twenty attachments took {scroll.Height}px against a {cap}px cap: the list is unbounded again, and the message underneath pays for it");
    });

    [HarnessFact]
    public void TheMessageItselfIsStillOnScreenUnderneath() => harness.Case(nameof(TheMessageItselfIsStillOnScreenUnderneath), app =>
    {
        // A fix that merely clipped the list would satisfy the case above and leave the body as
        // starved, so the consequence is asserted rather than the mechanism. PlainScroller, because
        // this fixture's body is text/plain and that is the control that draws it.
        app.OpenMessage(ManyAttachments);
        var body = app.Steady(app.Agent.Wait(Filter.ById("PlainScroller") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("no PlainScroller: the reading pane never drew this message's body"));
        var floor = BodyFloorDip * Input.Scale(body.Window);
        Assert.True(body.Rendered && body.Height > floor,
            $"the message body got {body.Height}px beside twenty attachments (offscreen: {body.Offscreen}); the message is what the list describes, and it must not be squeezed out of the pane");
    });
}
