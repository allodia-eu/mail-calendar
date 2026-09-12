# Forwarding a message opens the composer holding the files the original carries, as ordinary
# attachments that can be taken off again (docs/sending.md, "What a forward carries").
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. Every rule below lives in `ComposerView.xaml.cs`, which
# links WinUI, so that assembly cannot reach any of it. The seeded list is the sharp edge: the
# ListView is not bound in XAML, so the rows exist only once `AttachmentList.ItemsSource` has been
# re-assigned, and a seed that filled `_attachments` and skipped that assignment would leave a
# composer the user can neither see the files in nor remove one from, while the message still went
# out carrying them. Nothing headless sees the difference. Neither does the draft baseline: it is a
# field compared against a live count, and reading it means asking the running window.
#
# WHY THE HARNESS. Staging decodes the original's parts out of the raw source the engine fetched, so
# it is exactly the round trip the showcase engine does not perform. `docker/stalwart/seed.sh`
# appends `04-attachment.eml`, one `text/csv` part named by its sender, which is what makes the
# name assertion below an assertion about the sender's name rather than about a path.
#
# NOTHING BELOW MATCHES A LOCALISED STRING: the subject and the file name come from the seed, and
# every handle is an AutomationId.

$ForwardSubject = 'Message with a text attachment'
# The name the sender put on the part. The staged copy on disk is NOT called this (the core
# uniquifies it, so two composers cannot write over each other), which is the point of asserting
# on it: the row must carry the sender's name, never wherever the bytes happen to sit.
$ForwardFileName = 'report.csv'

# Opens the forward composer for the seeded message and waits for it. Returns nothing: every case
# below reads the composer off the window rather than through a handle, because a handle to a
# control that was rebuilt is the stale-element trap in uia.ps1.
function Open-ForwardComposer {
  Invoke-UiaElement (Get-MailRowByTitle $ForwardSubject)
  $forward = Wait-UiaElement -AutomationId 'ReadingForward' -TimeoutSec 30
  if (-not $forward) { throw 'no ReadingForward within 30s, the reading pane never drew its action row' }
  Invoke-UiaElement $forward
  # The composer opens AFTER staging, never before (docs/sending.md), so this wait covers the file
  # being decoded and written as well as the pane being built. It is generous for that reason.
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) {
    throw 'no composer within 30s of pressing Forward'
  }
}

# Put the composer away between cases. Cancel never asks, whatever the draft holds, so this is safe
# to call after a case that made one dirty.
function Close-ForwardComposer {
  $cancel = Find-UiaElement -AutomationId 'CancelButton' -Type Button
  if ($cancel) { Invoke-UiaElement $cancel }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
}

# The rows actually on screen in the composer's attachment list, scoped to that list. Scoped for
# uia.ps1 trap 2: a bare -Name sweep for the file name also matches the reading pane's own
# attachment chip for the same message, which is still in the tree behind the composer, and that
# one would pass this suite with the composer's list completely empty.
function Get-ComposerAttachments {
  $list = Find-UiaElement -AutomationId 'AttachmentList'
  if (-not $list) { throw 'the composer draws no AttachmentList at all' }
  # Comma, not bare `@(…)`: an empty array returned from a PowerShell function collapses to $null,
  # and the caller's `[0]` then fails with "Cannot index into a null array", which names neither the
  # list nor the rule. The one-element wrapper survives the pipeline and unrolls to the array.
  , @(Find-UiaElements -Type 'ListItem' -Root $list)
}

# The one row a forward of the seeded message must be holding, or a failure that says so. Every case
# that acts on the staged file goes through this rather than indexing, so an empty list is reported
# as the defect it is instead of as a scripting error.
function Get-TheStagedAttachment {
  $rows = Get-ComposerAttachments
  if ($rows.Count -ne 1) {
    throw "the forward composer holds $($rows.Count) attachment row(s), expected the one file the original carries; there is nothing here to select or remove"
  }
  $rows[0]
}

# A message row that is NOT the one being forwarded, for the cases that leave the composer by
# opening something else.
function Get-OtherMailRow {
  $row = Get-MailRows | Where-Object {
    -not (Get-UiaTree $_ | Where-Object { $_.Current.Name -like $ForwardSubject })
  } | Select-Object -First 1
  if (-not $row) { throw "the list holds only '$ForwardSubject', so there is nothing else to open" }
  $row
}

# The open ContentDialog, or $null. Identified by the CloseButton every ContentDialog carries, the
# same handle EventSeriesScope uses, rather than by any of its copy.
function Find-OpenDialog {
  param([int] $SettleMs = 1500)
  Wait-UiaQuiet -CapMs $SettleMs
  Find-UiaElement -AutomationId 'CloseButton' -Type Button
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = "a forward opens holding the original's file, under the name its sender gave it"
      Body = {
        Open-ForwardComposer
        try {
          $rows = Get-ComposerAttachments
          Assert-Equal 1 $rows.Count `
            "the forward composer shows $($rows.Count) attachment row(s) for a message carrying one file: an empty list is not a neutral state, it is the claim that the original had nothing attached"
          Assert-Equal $ForwardFileName $rows[0].Current.Name `
            "the attachment row reads '$($rows[0].Current.Name)': the row must carry the name the sender put on the part, not the uniquified path the core staged the bytes to, which is what the recipient would then see"
        }
        finally { Close-ForwardComposer }
      }
    },
    @{
      Name = 'the file is removable, like anything else the user attached'
      Body = {
        Open-ForwardComposer
        try {
          $remove = Find-UiaElement -AutomationId 'RemoveAttachmentButton' -Type Button
          if (-not $remove) { throw 'the composer offers no Remove button' }
          Assert-True ($remove.Current.IsEnabled -eq $false) `
            'Remove is live before any row is selected, so pressing it says nothing about which file it meant'
          # Selecting is what enables Remove. A ListViewItem here supports SelectionItem and not
          # Invoke, which Invoke-UiaElement falls back to on its own.
          Invoke-UiaElement (Get-TheStagedAttachment)
          $remove = Find-UiaElement -AutomationId 'RemoveAttachmentButton' -Type Button
          Assert-True ($remove.Current.IsEnabled -eq $true) `
            'selecting the staged file did not enable Remove, so a forward imposes its files rather than proposing them'
          Invoke-UiaElement $remove
          Assert-Equal 0 (Get-ComposerAttachments).Count `
            'the row survived Remove: a staged file that cannot be taken off goes out whether or not the sender meant to pass it on'
        }
        finally { Close-ForwardComposer }
      }
    },
    @{
      Name = 'an untouched forward is not a draft, so abandoning it asks nothing'
      Body = {
        Open-ForwardComposer
        # Opening another message is the one route that puts the "Discard draft?" question up, the
        # composer being a pane rather than a modal (MailListView.MayOpenMessageAsync).
        Invoke-UiaElement (Get-OtherMailRow)
        $dialog = Find-OpenDialog
        if ($dialog) {
          # Leave the screen clean before failing: Discard, so no composer is left for the next case.
          $discard = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button
          if ($discard) { Invoke-UiaElement $discard }
          throw 'a forward nobody typed into was treated as an unsent draft. The staged files are still in the mailbox, so there is nothing to lose and nothing to ask about; the guard is measuring the attachment count against zero rather than against what the composer opened with'
        }
        Assert-True (Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10) `
          'the composer stayed up after the other message was opened, so the click read as having done nothing'
      }
    },
    @{
      Name = 'taking one off IS a decision about what goes out, and does ask'
      Body = {
        Open-ForwardComposer
        Invoke-UiaElement (Get-TheStagedAttachment)
        Invoke-UiaElement (Find-UiaElement -AutomationId 'RemoveAttachmentButton' -Type Button)
        Invoke-UiaElement (Get-OtherMailRow)
        $dialog = Find-OpenDialog
        try {
          Assert-True ($null -ne $dialog) `
            'removing a forwarded file and then walking away lost that decision in silence: the count changed, which is exactly what the guard is there to notice'
        }
        finally {
          # Discard, so the case leaves no composer behind: the primary button is Discard and the
          # close button is "Keep editing" (MainWindow.Compose.cs).
          $discard = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button
          if ($discard) { Invoke-UiaElement $discard }
        }
      }
    }
  )
}
