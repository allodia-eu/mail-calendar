# The contacts editor, driven up to the point of a write and no further (docs/contacts.md §3).
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. Everything the editor *decides* lives in the pure
# ContactEditing, which that assembly links and gates. What it cannot see is anything WinUI-shaped,
# and this file was written against three such failures, two of which shipped:
#
#   * The create affordance is hidden unless the core reports a WRITABLE address book, so its
#     presence is the JMAP write-destination binding working. A binding that broke would read as a
#     product decision ("this server refuses writes") rather than as a fault.
#   * Each repeating value row carried its own TextBox.Header, so the field's name was drawn twice
#     and the row's remove button, centred on the whole cell, sat level with the label instead of
#     with the box it removes.
#   * The write status was inside the DETAIL pane, which is collapsed while nobody is open, which
#     is exactly the state a create is made from. So a create said nothing at all, and `Failed`,
#     which means "we could not confirm this saved" rather than "rejected", could never be read.
#
# WHY IT STOPS BEFORE THE WRITE. This suite shares the harness with everything else and the app
# offers no delete, so an actual create would leave a card behind and accumulate one per run. The
# write itself is gated in `mailcal-app`'s contacts suite against a real engine, and was driven by
# hand against the harness with a JMAP read-back.
#
# WHAT THAT COSTS, SAID OUT LOUD. The third failure above is therefore NOT gated here. The status
# line is collapsed until a write settles, and a collapsed element is absent from the automation
# tree entirely, so there is nothing to assert on until something has been saved. It is guarded by
# the rule in docs/contacts.md §3 and was verified by hand. Gating it would need a MAILCAL_* hook
# that stages a ContactWriteStatus in the CORE, on the InvitationReplyPrompt.Tests pattern: fake
# the input, never the surface.
#
# WHY HARNESS. The create affordance is a real capability read off a real server. The showcase
# engine would answer for a mailbox that performs no writes.
#
# EVERY SELECTOR HERE IS AN AutomationId. The editor's labels are localised, so a suite that
# matched one would assert the app's language as much as its layout, and would go red in Dutch.

<#
.SYNOPSIS
The open ContentDialog, as an element, never the whole window.
.DESCRIPTION
Identified by the CloseButton every ContentDialog carries, not as "the first Popup": the shell
keeps a second, empty Popup that sorts ahead of it, and taking that one hands back an element with
nothing in it, under which every "X is not on screen" assertion passes for the wrong reason. (Each
suite defines its own copy: the runner loads one file at a time under -Filter.)
#>
function Get-EditorDialog {
  param([int] $TimeoutSec = 20)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $popup = Get-UiaTree |
      Where-Object {
        $_.Current.ClassName -eq 'Popup' -and
        $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window -and
        (Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $_)
      } | Select-Object -First 1
    if ($popup) { return $popup }
    Start-Sleep -Milliseconds 250
  }
  throw 'no dialog appeared'
}

function Close-EditorDialog {
  $close = Find-UiaElement -AutomationId 'CloseButton' -Type Button
  if ($close) { Invoke-UiaElement $close -SettleMs 900 }
}

# Put the shell on Contacts, idempotently (the dataset launches on mail).
function Show-ContactsSurface {
  $list = Find-UiaElement -AutomationId 'ContactsList'
  if ($list) { return $list }
  $nav = Find-UiaElement -AutomationId 'NavContacts'
  if (-not $nav) { throw 'no Contacts entry in the navigation pane' }
  Invoke-UiaElement $nav -SettleMs 1500
  $list = Wait-UiaElement -AutomationId 'ContactsList' -TimeoutSec 60
  if (-not $list) { throw 'the contacts list did not come up' }
  return $list
}

# The create button, asserted present rather than returned $null: its absence is the interesting
# failure, and every case below needs it to reach the form at all.
function Get-NewContactButton {
  $new = Wait-UiaElement -AutomationId 'ContactsNew' -Type Button -TimeoutSec 60
  if (-not $new) {
    throw ('no New contact button (#ContactsNew). It is hidden unless the core reports a WRITABLE ' +
      'address book, so either the harness account lost its contacts destination or the JMAP ' +
      'adapter was left unbound (docs/contacts.md §3). Check scripts/dev/harness.sh status first.')
  }
  return $new
}

# Open the create form and return the dialog.
function Open-CreateForm {
  Show-ContactsSurface | Out-Null
  Invoke-UiaElement (Get-NewContactButton) -SettleMs 1800
  $dialog = Get-EditorDialog
  if (-not (Find-UiaElement -AutomationId 'ContactGivenName' -Type Edit -Root $dialog)) {
    throw 'the dialog on screen is not the contact editor (no #ContactGivenName)'
  }
  return $dialog
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'the create affordance is offered at all'
      Body = {
        Show-ContactsSurface | Out-Null
        $new = Get-NewContactButton
        Assert-True $new.Current.IsEnabled `
          'the New contact button is on screen but disabled, so the core offered a destination the form cannot reach'
      }
    },
    @{
      Name = 'an empty form is refused, and says why, rather than closing'
      Body = {
        $dialog = Open-CreateForm
        try {
          $before = @(Find-UiaElements -Type Text -Root $dialog).Count
          $save = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $dialog
          if (-not $save) { throw 'the editor has no Save button' }
          Invoke-UiaElement $save -SettleMs 1500
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'ContactGivenName' -Type Edit)) `
            'Save closed the editor on an empty form. The core refuses a card with nothing to file it under, so the write is silently never made and the user is told nothing (docs/contacts.md §3)'
          # Counted rather than matched, because the sentence is localised: what must be true is
          # that the refusal PUT one on screen. A dialog that just sits there reads as a dead
          # button, and retrying the same form is refused the same way.
          $after = @(Find-UiaElements -Type Text -Root (Get-EditorDialog)).Count
          Assert-GreaterThan $before $after `
            'the editor refused the form without stating a reason under it'
        }
        finally { Close-EditorDialog }
      }
    },
    @{
      Name = 'a repeating row is a bare value box, with its remove button in line with it'
      Body = {
        $dialog = Open-CreateForm
        try {
          $boxes = @(Find-UiaElements -AutomationId 'ContactEmailValue' -Type Edit -Root $dialog)
          Assert-Equal 1 $boxes.Count `
            'the email field did not open on exactly one empty row, so the geometry below is measuring something else'
          $box = Get-RenderedBounds $boxes[0] -What 'the email value box'
          # Against a field that is SUPPOSED to carry a header, rather than a constant: the two
          # are the same control, so the only difference in height is the header, and the
          # comparison survives a DPI, theme or font change that a magic number would not.
          $headed = Get-RenderedBounds `
            (Find-UiaElement -AutomationId 'ContactGivenName' -Type Edit -Root $dialog) `
            -What 'the First name field, which does carry a header'
          Assert-True ($box.Height -lt $headed.Height) `
            "a repeating row's value box is $($box.Height)px tall, as tall as the headed First name field. So it carries a header of its own: the field's name is drawn twice, and the row's remove button, which centres on the whole cell, ends up level with that label instead of with the box"
          # A second regression, and the reason the height check above is not enough on its own:
          # this one catches the button being pinned to the top or the bottom of the row. It
          # CANNOT catch the header, because a TextBox's bounds include its header, so a headed
          # box and its button share a centre and the drift stays 0. Verified by breaking it.
          $remove = Find-UiaElement -AutomationId 'ContactEmailRemove' -Type Button -Root $dialog
          if (-not $remove) { throw 'the email row has no remove button (#ContactEmailRemove)' }
          $button = Get-RenderedBounds $remove -What 'the email row remove button'
          $drift = [Math]::Abs(($box.Y + $box.Height / 2) - ($button.Y + $button.Height / 2))
          # Physical pixels, so the tolerance is converted from DIPs rather than written flat: at
          # 200% a 4 DIP allowance is 8 px, and a flat 4 would fail a correct layout there.
          Assert-True ($drift -le (ConvertTo-UiaPixels 4)) `
            "the remove button's centre is $drift px off the value box's, so it is not vertically centred on the value it removes"
        }
        finally { Close-EditorDialog }
      }
    },
    @{
      Name = 'a value row is labelled once, not twice'
      Body = {
        $dialog = Open-CreateForm
        try {
          # The section heading names the field; a TextBox.Header on each row repeats it down the
          # form. Read off the row's own subtree so the heading, which is a sibling, is not counted.
          $box = Find-UiaElement -AutomationId 'ContactEmailValue' -Type Edit -Root $dialog
          if (-not $box) { throw 'the email row has no value box (#ContactEmailValue)' }
          $labels = @(Find-UiaElements -Type Text -Root $box)
          Assert-Equal 0 $labels.Count `
            "the email value box carries $($labels.Count) label(s) of its own. The heading above the field already names it, so a header on the row draws it twice"
        }
        finally { Close-EditorDialog }
      }
    }
  )
}
