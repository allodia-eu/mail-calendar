#!/usr/bin/env pwsh
# The composer's header is one line per field, the label in a column beside it (ComposerView.Header.cs):
# From, To, Subject take three lines, not six, so the editor starts where a mail client's would.
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. Where the pills and the input go is pinned there
# (FlowLayoutTests), but whether the label sits beside the field or above it is XAML, and a header
# that went back to labels over fields would pass every headless suite.
#
# WHY SHOWCASE. Nothing here sends: a composer is opened, measured, and cancelled.
#
# Measured on Subject, whose input starts where its line does; To's starts after its pills.

function Open-HeaderComposer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) { throw 'no composer within 30s of pressing Compose' }
}

function Close-HeaderComposer {
  $cancel = Find-UiaElement -AutomationId 'CancelButton'
  if ($cancel) { Invoke-UiaElement $cancel }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
}

# The label column is at least a short label wide; a field with its label above it starts at the
# editor's own left edge.
$LabelColumnDip = 24
# One line of text box, with room to spare; a label stacked over the box adds a line of text to it.
$OneLineDip = 40

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a header field sits beside its label, on one line'
      Body = {
        Open-HeaderComposer
        try {
          $subject = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'SubjectBox') -What 'the Subject field'
          $editor = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'Editor') -What 'the editor'
          $indent = $subject.X - $editor.X
          Assert-True ($indent -ge (ConvertTo-UiaPixels $LabelColumnDip)) `
            "Subject's field starts ${indent}px right of the editor's edge: its label is above it rather than beside it"
          Assert-True ($subject.Height -le (ConvertTo-UiaPixels $OneLineDip)) `
            "Subject's field is $($subject.Height)px tall: more than one line, which is a label stacked over the box"
          $to = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ToField') -What 'the To field'
          $gap = $subject.Y - ($to.Y + $to.Height)
          Assert-True ($gap -le (ConvertTo-UiaPixels 12)) `
            "${gap}px between the To and Subject lines: something is drawn between two header rows"
        }
        finally { Close-HeaderComposer }
      }
    }
  )
}
