# A message with more attachments than fit must not push its own body off the screen.
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. The rule is a layout one: the attachment panel sits in
# an `Auto` row ABOVE the body's `*` row, so an unbounded list takes what it wants and starves the
# message. Nothing that cannot link WinUI can see a row height, and nothing headless can see that
# the body ended up with none.
#
# WHY THE HARNESS. The showcase seeds no message with enough attachments to reach the cap, and one
# attachment cannot tell a capped list from an uncapped one, which is exactly why the defect
# shipped on three clients. `docker/stalwart/seed.sh` appends `10-many-attachments.eml` (twenty
# files) for this.
#
# NOTHING BELOW MATCHES A LOCALISED STRING: the subject comes from the seed, and both handles are
# AutomationIds.

$ManyAttachments = 'Message with many attachments'
# Both numbers are DEVICE-INDEPENDENT pixels, as XAML is written, and are converted at assert time:
# BoundingRectangle answers in physical ones, so on a 200% display an unconverted comparison reads
# the cap as breached at exactly twice its value, and the body floor as met at half of it
# (ConvertTo-UiaPixels in uia.ps1).
#
# The cap in ReadingView.xaml, plus a little slack for the ScrollViewer's own chrome, the assertion
# is "bounded", not "exactly this", and pinning the pixel would fail on the next padding change.
$CapDipWithSlack = 200
# What is left for the message beside a full attachment bar. A few lines of mail, not a sliver.
$BodyFloorDip = 120

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'twenty attachments stop at the cap instead of taking the pane'
      Body = {
        Invoke-UiaElement (Get-MailRowByTitle $ManyAttachments)
        $scroll = Wait-UiaElement -AutomationId 'AttachmentScroll' -TimeoutSec 30
        if (-not $scroll) {
          throw "no AttachmentScroll within 30s, either the body never arrived, or the attachment panel is not the ScrollViewer this rule depends on"
        }
        $height = (Get-RenderedBounds -Element $scroll -What 'the attachment list').Height
        $cap = ConvertTo-UiaPixels $CapDipWithSlack
        Assert-True ($height -le $cap) `
          "twenty attachments took ${height}px against a ${cap}px cap: the list is unbounded again, and it is the message underneath that pays for it"
      }
    },
    @{
      Name = 'the message itself is still on screen underneath'
      Body = {
        # The half a cap alone does not buy. A "fix" that merely clipped the list would satisfy the
        # case above while leaving the body exactly as starved, so the consequence is asserted
        # rather than the mechanism.
        #
        # PlainScroller, not the body row's Grid: a bare layout panel gets no automation peer, so an
        # AutomationId on one is unreachable and a test waiting for it can only ever time out. This
        # fixture's body is text/plain, which is the control that draws it.
        Invoke-UiaElement (Get-MailRowByTitle $ManyAttachments)
        $body = Wait-UiaElement -AutomationId 'PlainScroller' -TimeoutSec 30
        if (-not $body) { throw "no PlainScroller within 30s, the reading pane never drew this message's body" }
        # Through Get-RenderedBounds: a body pushed off the surface reports infinities rather than a
        # small height, and that is precisely the shape of the defect this case is here to catch.
        $height = (Get-RenderedBounds -Element $body -What "the message body").Height
        Assert-GreaterThan (ConvertTo-UiaPixels $BodyFloorDip) $height `
          "the message body got ${height}px beside twenty attachments, the message is what the attachment list is there to describe, and it must not be squeezed out of the pane"
      }
    }
  )
}
