#!/usr/bin/env pwsh
# Print, from the reading pane's overflow menu (docs/reading-actions.md, "Printing a message").
#
# WHAT ONLY THIS SUITE CAN SEE. The page is built and unit-tested in `mailcal-app`; what is native
# here is a hidden WebView2 per print and the system dialog it opens, and two of their failures are
# invisible to an assembly without WinUI:
#
#   * The dialog prints the page its web view holds when the reader presses Print IN THE DIALOG,
#     not the page it opened on, and it is not modal to the app. A web view shared between prints
#     lets a second Print replace the page under an open dialog, which then fails with the system's
#     "Print failed" while the app says nothing.
#   * Without a <title>, WebView2 names the job after the page's URL, which for a page loaded from
#     a string is the whole message in base64: in the print queue and in a saved PDF.
#
# HOW IT READS A PRINT. Through "Microsoft Print to PDF", into a file, never a real printer. Its
# PDF carries the job's name as /Title, which is the one thing in the file that says WHICH page was
# printed (it writes glyphs with no text layer to search). So the last case asserts the subject of
# the message the dialog was opened for, which is the overlap and the title in one assertion.
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
Every open system print dialog.
.DESCRIPTION
Recognised by its printer picker's automation id rather than by its title, which is localised and
names WebView2 rather than this app. The dialog is a top-level window of the shell, not a child of
ours.
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
WebView2's own message boxes ("Print failed"), each a RootView window of its browser process.
#>
function Get-WebViewMessageBoxes {
  $ids = @(Get-Process msedgewebview2 -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
  $condition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'RootView')
  @([System.Windows.Automation.AutomationElement]::RootElement.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants, $condition) |
    Where-Object { $ids -contains $_.Current.ProcessId -and $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window })
}

function Close-WebViewMessageBoxes {
  foreach ($box in Get-WebViewMessageBoxes) {
    $button = Get-UiaTree -Root $box | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button } | Select-Object -First 1
    if ($button) { $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
  }
  Start-Sleep -Milliseconds 500
}

<#
.SYNOPSIS
Put away every dialog a print case may leave: message boxes, the PDF save dialog, print dialogs.
#>
function Close-PrintDialogs {
  Close-WebViewMessageBoxes
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

# The PDF printer's own Save As, a #32770 of WebView2's browser process owned by our window.
function Get-PrintSaveDialogs {
  $ids = @(Get-Process msedgewebview2 -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
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
      Name = 'Print opens the system dialog, and cancelling it reports nothing'
      Body = {
        Open-PrintMessage $PrintFirstSubject
        try {
          Invoke-PrintMenuItem
          $dialog = Wait-SystemPrintDialog
          (Find-InPrintDialog -Dialog $dialog -AutomationId 'CloseButton').GetCurrentPattern(
            [System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          Start-Sleep -Seconds 2
          $line = Find-UiaElement -AutomationId 'ExportError'
          $shown = $line -and -not [double]::IsInfinity($line.Current.BoundingRectangle.X) -and $line.Current.BoundingRectangle.Height -gt 0
          Assert-True (-not $shown) "a cancelled print is the reader's choice, not a failure; the pane said '$($line.Current.Name)'"
        } finally {
          Close-PrintDialogs
        }
      }
    },
    @{
      Name = 'a second Print while a dialog is open leaves that dialog its own page, named by its subject'
      Body = {
        $destination = Join-Path $env:TEMP ("mailcal-uitest-print-" + [guid]::NewGuid().ToString('N') + ".pdf")
        Open-PrintMessage $PrintFirstSubject
        try {
          Invoke-PrintMenuItem
          $dialog = Wait-SystemPrintDialog
          # The dialog is not modal to the app, so a reader can do exactly this.
          Open-PrintMessage $PrintSecondSubject
          Invoke-PrintMenuItem
          Start-Sleep -Seconds 4
          # WebView2 opens one print dialog at a time and answers the second request with a
          # message box of its own. That is its to say; what this case guards is the first dialog.
          Close-WebViewMessageBoxes
          $print = Select-PdfPrinterIn -Dialog $dialog
          $print.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          Save-PrintedPdf -Destination $destination
          Assert-Equal $PrintFirstSubject (Get-PdfTitle -Path $destination) `
            'the dialog must print the message it was opened for, and the job is named by its subject rather than by the page URL (docs/reading-actions.md)'
        } finally {
          Close-PrintDialogs
          Remove-Item $destination -ErrorAction SilentlyContinue
        }
      }
    }
  )
}
