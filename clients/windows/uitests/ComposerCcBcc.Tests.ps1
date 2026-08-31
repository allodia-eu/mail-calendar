# The composer opens on the header a message usually needs, From, To, Subject, with Cc and Bcc
# behind the chevron on the To row (docs/composer-security.md, Gate 12).
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. That assembly reaches the rule that decides whether the
# row opens revealed (`RecipientTokens.RevealsCcBcc`, unit-tested there) and nothing else: it cannot
# link WinUI, so it cannot see whether anything is actually assigned to `CcBccPanel.Visibility`, nor
# whether the toggle is wired to it. A composer that opened with the row already showing, or one
# whose chevron did nothing, would pass every headless suite in the repo.
#
# WHY SHOWCASE. Nothing here sends: a composer is opened, read, and cancelled. It does need an
# account to send from, which the showcase seeds deterministically.
#
# The two fields are reached by the automation id on their INPUT, not by their label: the labels are
# localised, and the three fields are one control used three times, so nothing else tells them apart.
# A collapsed field is absent from the automation tree entirely, which is what makes "Cc is not on
# screen" something this can fail on rather than assume.

# Opens a new-message composer and waits for it.
function Open-Composer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose
  $send = Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30
  if (-not $send) { throw 'no composer within 30s of pressing Compose' }
}

function Close-Composer {
  $cancel = Find-UiaElement -Name 'Cancel' -Type Button
  if ($cancel) { Invoke-UiaElement $cancel }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
}

# How far the chevron's centre may sit from the To input's centre, in DEVICE-INDEPENDENT pixels as
# XAML is written. BoundingRectangle answers in physical ones, so this is converted at assert time
# (ConvertTo-UiaPixels in uia.ps1), unconverted, a 200% display reads a 7epx drift as within a
# 4px budget and the assertion passes on exactly the defect it exists to catch.
#
# Small on purpose. The bug this pins drew the chevron 4epx low, because it was aligned against the
# whole field rather than against its input: the field's StackPanel spaces the closed suggestion
# popup after the input, so the field's bottom edge is not the input's.
$CentreToleranceDip = 2

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a new message opens with Cc and Bcc collapsed'
      Body = {
        Open-Composer
        try {
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'ToField')) `
            'the To field is missing: the composer opened without the one recipient field it always shows'
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'CcField')) `
            'Cc is on screen before anyone asked for it, the header is meant to open as From, To, Subject'
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'BccField')) `
            'Bcc is on screen before anyone asked for it, the header is meant to open as From, To, Subject'
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'the chevron is drawn level with the To input, not with the field around it'
      Body = {
        Open-Composer
        try {
          $input = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ToField') -What 'the To input'
          $chevron = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'CcBccToggle') -What 'the Cc/Bcc chevron'
          $drift = [Math]::Abs(($chevron.Y + $chevron.Height / 2) - ($input.Y + $input.Height / 2))
          $budget = ConvertTo-UiaPixels $CentreToleranceDip
          Assert-True ($drift -le $budget) `
            "the chevron's centre is ${drift}px off the To input's, against a ${budget}px budget: it is being aligned against something other than the input it belongs to"
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'the chevron reveals Cc and Bcc, and puts them away again'
      Body = {
        Open-Composer
        try {
          # A ToggleButton exposes TogglePattern, not Invoke, Set-UiaToggle reads the state first,
          # so this sets rather than cycles (uia.ps1).
          $toggle = Find-UiaElement -AutomationId 'CcBccToggle'
          if (-not $toggle) { throw 'no CcBccToggle, the composer draws no way to reach Cc and Bcc at all' }
          Set-UiaToggle $toggle -On
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'CcField')) `
            'the chevron is checked and Cc still is not on screen: the toggle is not wired to the fields'
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'BccField')) `
            'the chevron is checked and Bcc still is not on screen: the toggle is not wired to the fields'
          Set-UiaToggle $toggle
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'CcField')) `
            'Cc stayed on screen after the chevron was unchecked, the reveal is one-way'
        }
        finally { Close-Composer }
      }
    }
  )
}
