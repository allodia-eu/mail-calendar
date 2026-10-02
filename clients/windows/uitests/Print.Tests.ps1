#!/usr/bin/env pwsh
# Print, from the reading pane's overflow menu (docs/reading-actions.md, "Printing a message").
#
# WHAT ONLY THIS SUITE CAN SEE. The page is built and unit-tested in `mailcal-app`; what is native
# here is the hidden WebView2 that lays it out and Windows' print dialog, opened for this window
# through PrintManager. Three properties of that dialog are invisible to an assembly without WinUI:
#
#   * It belongs to the window Print was pressed in. A dialog that does not (WebView2's own system
#     dialog is a window of the web view's process) can open BEHIND the app, and every later press
#     then does nothing, because a window has one print dialog at a time.
#   * It previews the pages this client hands it, rather than "This app doesn't support print
#     preview".
#   * The job is named by the subject, and prints the message it was opened for.
#
# HOW IT READS A PRINT. Through "Microsoft Print to PDF", into a file, never a real printer. The
# PDF's /Title is the job's name, and it is the one thing in the file that says WHICH message was
# printed: the pages are images, so there is no text to search.
#
# WHY THE HARNESS. The body has to have arrived for Print to be offered, and the seeded subjects
# are stable strings. Nothing here writes mail.

$PrintFirstSubject = 'Harness baseline message'
$PrintSecondSubject = 'Fixed-width newsletter that cannot reflow'
$PrintPdfPrinter = 'Microsoft Print to PDF'

<#
.SYNOPSIS
Open a seeded message by subject and wait for its action row.
#>
function Open-PrintMessage {
  param([Parameter(Mandatory)] [string] $Subject)
  Invoke-UiaElement (Get-MailRowByTitle $Subject)
  $null = Wait-UiaElement -AutomationId 'ReadingOverflow' -TimeoutSec 30
  # The subject on screen is the evidence the open finished; Print is decided on the body after it.
  for ($i = 0; $i -lt 40; $i++) {
    if ((Find-UiaElement -AutomationId 'SubjectText').Current.Name -eq $Subject) { return }
    Start-Sleep -Milliseconds 250
  }
  throw "the reading pane never showed '$Subject'"
}

<#
.SYNOPSIS
Expand the overflow and return the Print item, enabled or not.
#>
function Get-PrintMenuItem {
  $overflow = Find-UiaElement -AutomationId 'ReadingOverflow'
  $overflow.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
  $item = Wait-UiaElement -AutomationId 'ReadingPrint' -TimeoutSec 10
  if (-not $item) { throw 'the overflow offered no Print item' }
  $item
}

function Close-PrintMenu {
  $overflow = Find-UiaElement -AutomationId 'ReadingOverflow'
  try { $overflow.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Collapse() } catch { }
  Start-Sleep -Milliseconds 400
}

function Invoke-PrintMenuItem {
  $item = Get-PrintMenuItem
  Assert-True $item.Current.IsEnabled 'Print must be offered once the body has arrived'
  $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

<#
.SYNOPSIS
Every open print dialog.
.DESCRIPTION
Recognised by its printer picker's automation id rather than by its title, which is localised. The
dialog is a top-level window of the shell, tied to ours as its modal owner, not a child of it.
#>
function Get-SystemPrintDialogs {
  $condition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'ApplicationFrameWindow')
  $frames = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children, $condition)
  @($frames | Where-Object {
      Get-UiaTree -Root $_ | Where-Object { $_.Current.AutomationId -eq 'printerSelector' } | Select-Object -First 1
    })
}

function Wait-SystemPrintDialog {
  param([int] $TimeoutSec = 60)
  # A minute, because the dialog first waits on the default printer, and a network printer that is
  # off keeps it on "Waiting for printer connection" for a good while before it gives up.
  for ($i = 0; $i -lt $TimeoutSec; $i++) {
    $dialog = Get-SystemPrintDialogs | Select-Object -First 1
    if ($dialog) { return $dialog }
    Start-Sleep -Seconds 1
  }
  throw "no system print dialog within ${TimeoutSec}s of pressing Print"
}

function Find-InPrintDialog {
  param([Parameter(Mandatory)] $Dialog, [Parameter(Mandatory)] [string] $AutomationId)
  Get-UiaTree -Root $Dialog | Where-Object { $_.Current.AutomationId -eq $AutomationId } | Select-Object -First 1
}

<#
.SYNOPSIS
Put away every dialog a print case may leave: message boxes, the PDF save dialog, print dialogs.
#>
function Close-PrintDialogs {
  foreach ($save in Get-PrintSaveDialogs) {
    $cancel = Get-UiaTree -Root $save | Where-Object { $_.Current.AutomationId -eq '2' -and $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button } | Select-Object -First 1
    if ($cancel) { $cancel.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
  }
  foreach ($dialog in Get-SystemPrintDialogs) {
    $cancel = Find-InPrintDialog -Dialog $dialog -AutomationId 'CloseButton'
    if ($cancel) { $cancel.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
  }
  for ($i = 0; $i -lt 20 -and ((Get-SystemPrintDialogs).Count -gt 0 -or (Get-PrintSaveDialogs).Count -gt 0); $i++) {
    Start-Sleep -Milliseconds 500
  }
}

<#
.SYNOPSIS
Choose the PDF printer in an open dialog and wait until it will print.
#>
function Select-PdfPrinterIn {
  param([Parameter(Mandatory)] $Dialog)
  $picker = Find-InPrintDialog -Dialog $Dialog -AutomationId 'printerSelector'
  $picker.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
  Start-Sleep -Seconds 1
  $choice = Get-UiaTree -Root $Dialog | Where-Object {
    $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::ListItem -and $_.Current.Name -eq $PrintPdfPrinter
  } | Select-Object -First 1
  if (-not $choice) { throw "the print dialog does not offer '$PrintPdfPrinter', which this suite prints through" }
  $choice.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
  for ($i = 0; $i -lt 30; $i++) {
    $print = Find-InPrintDialog -Dialog $Dialog -AutomationId 'PrintButton'
    if ($print -and $print.Current.IsEnabled) { return $print }
    Start-Sleep -Milliseconds 500
  }
  throw "the dialog's Print button never enabled with '$PrintPdfPrinter' chosen"
}

# The PDF printer's own Save As: a #32770 of this app's process.
function Get-PrintSaveDialogs {
  $ids = @(Get-Process Mailcal -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
  $condition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ClassNameProperty, '#32770')
  @([System.Windows.Automation.AutomationElement]::RootElement.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants, $condition) |
    Where-Object { $ids -contains $_.Current.ProcessId })
}

<#
.SYNOPSIS
Type a path into the PDF printer's Save As and wait for the file. Keyboard only: its controls are
patternless (uia.ps1, Save-ThroughFilePicker).
#>
function Save-PrintedPdf {
  param([Parameter(Mandatory)] [string] $Destination)
  Add-Type -AssemblyName System.Windows.Forms
  $save = $null
  for ($i = 0; $i -lt 30 -and -not $save; $i++) {
    Start-Sleep -Seconds 1
    $save = Get-PrintSaveDialogs | Select-Object -First 1
  }
  if (-not $save) { throw "the PDF printer never asked where to save, so nothing was printed" }
  [void][Allodia.UiaDpi]::SetForegroundWindow([IntPtr] $save.Current.NativeWindowHandle)
  Start-Sleep -Milliseconds 600
  [System.Windows.Forms.SendKeys]::SendWait('%n')
  Start-Sleep -Milliseconds 300
  [System.Windows.Forms.SendKeys]::SendWait('^a')
  Start-Sleep -Milliseconds 300
  [System.Windows.Forms.SendKeys]::SendWait(($Destination -replace '([+^%~()\[\]{}])', '{$1}'))
  Start-Sleep -Milliseconds 400
  [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
  # Settled length rather than existence: the file appears empty first.
  $previous = -1
  for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 500
    $length = if (Test-Path $Destination) { (Get-Item $Destination).Length } else { -1 }
    if ($length -gt 0 -and $length -eq $previous) { return }
    $previous = $length
  }
  throw "nothing was written at $Destination"
}

<#
.SYNOPSIS
The /Title of a PDF the PDF printer wrote: a literal string, UTF-16BE behind a byte-order mark.
#>
function Get-PdfTitle {
  param([Parameter(Mandatory)] [string] $Path)
  $raw = [System.Text.Encoding]::Latin1.GetString([System.IO.File]::ReadAllBytes($Path))
  $match = [regex]::Match($raw, '/Title\s*\(((?:\\.|[^\\)])*)\)', 'Singleline')
  if (-not $match.Success) { return $null }
  $literal = [regex]::Replace($match.Groups[1].Value, '\\(.)', '$1')
  $bytes = [System.Text.Encoding]::Latin1.GetBytes($literal)
  if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF) {
    return [System.Text.Encoding]::BigEndianUnicode.GetString($bytes, 2, $bytes.Length - 2)
  }
  $literal
}

<#
.SYNOPSIS
Whether the mailbox window takes input. A modal dialog disables its owner.
#>
function Test-MailboxEnabled {
  (Get-MailcalWindow).Current.IsEnabled
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'Print is offered below the export once the body has arrived'
      Body = {
        Open-PrintMessage $PrintFirstSubject
        try {
          $print = Get-PrintMenuItem
          $export = Find-UiaElement -AutomationId 'ReadingExportEml'
          Assert-True $print.Current.IsEnabled 'Print must be offered once the body has arrived (docs/reading-actions.md)'
          Assert-True ($print.Current.BoundingRectangle.Y -gt $export.Current.BoundingRectangle.Y) `
            'Print sits below Save as .eml in the overflow, on every platform'
        } finally {
          Close-PrintMenu
        }
      }
    },
    @{
      Name = 'the dialog belongs to this window, previews the page and is named by the subject'
      Body = {
        Open-PrintMessage $PrintSecondSubject
        try {
          Invoke-PrintMenuItem
          $dialog = Wait-SystemPrintDialog
          Assert-True (-not (Test-MailboxEnabled)) `
            'the print dialog must hold the window it was opened from, which is what keeps it in front of the app; the mailbox still takes input, so the dialog is not its own (docs/reading-actions.md)'
          Assert-True ($dialog.Current.Name.StartsWith($PrintSecondSubject)) `
            "the dialog is titled by the job's name, which must be the subject; it read '$($dialog.Current.Name)'"
          # The dialog says so in this element when the app hands it no preview pages.
          $noPreview = Find-InPrintDialog -Dialog $dialog -AutomationId 'NoPreviewAvailableText'
          Assert-True ($null -eq $noPreview) "the dialog shows no preview: '$($noPreview.Current.Name)'"
          (Find-InPrintDialog -Dialog $dialog -AutomationId 'CloseButton').GetCurrentPattern(
            [System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          for ($i = 0; $i -lt 20 -and -not (Test-MailboxEnabled); $i++) { Start-Sleep -Milliseconds 250 }
          Assert-True (Test-MailboxEnabled) 'the window must take input again once the dialog is cancelled'
          $line = Find-UiaElement -AutomationId 'ExportError'
          $shown = $line -and -not [double]::IsInfinity($line.Current.BoundingRectangle.X) -and $line.Current.BoundingRectangle.Height -gt 0
          Assert-True (-not $shown) "a cancelled print is the reader's choice, not a failure; the pane said '$($line.Current.Name)'"
        } finally {
          Close-PrintDialogs
        }
      }
    },
    @{
      Name = 'the job prints the message it was opened for'
      Body = {
        $destination = Join-Path $env:TEMP ("mailcal-uitest-print-" + [guid]::NewGuid().ToString('N') + ".pdf")
        Open-PrintMessage $PrintFirstSubject
        try {
          Invoke-PrintMenuItem
          $dialog = Wait-SystemPrintDialog
          $print = Select-PdfPrinterIn -Dialog $dialog
          $print.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          Save-PrintedPdf -Destination $destination
          Assert-Equal $PrintFirstSubject (Get-PdfTitle -Path $destination) `
            'the printed job must be the message Print was pressed on, named by its subject (docs/reading-actions.md)'
        } finally {
          Close-PrintDialogs
          Remove-Item $destination -ErrorAction SilentlyContinue
        }
      }
    }
  )
}
