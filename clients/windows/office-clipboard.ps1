#!/usr/bin/env pwsh
# Puts what Word or Excel really writes on the clipboard, for testing a paste.
#
#   ./office-clipboard.ps1 -From Word     # a formatted document, copied whole
#   ./office-clipboard.ps1 -From Excel    # A1:B3 with formatted cells, copied
#   ./office-clipboard.ps1 -Close         # after the paste: close both, unsaved
#
# A hand-built clipboard answers only the question its author already had. Word and Excel each
# write several flavours at once (HTML, RTF, plain text and a picture of the selection), and the
# HTML carries markup no fixture writer would think of: Excel names a cell's bold as a CLASS in a
# style sheet (`.xl63 {font-weight:700}`), not on the cell. Both are driven over COM, unseen, and
# print the formats they left on the clipboard.
#
# Traps, each found by hitting it:
#   * Excel renders its clipboard only when something asks, so it must still be running when the
#     paste happens. That is why copying and closing are two calls, with the paste between them.
#   * Closing Excel can empty the clipboard: a paste straight after -Close found nothing to paste.
#   * -Close cannot reach the instances through COM: Marshal.GetActiveObject is not in PowerShell 7,
#     and an unseen automation instance is not one a user could be holding. So it stops the processes
#     started as automation servers (`/Automation -Embedding`) and nothing else: a Word or Excel the
#     user opened is never one of them. Their documents are unsaved throwaways.
#   * Characters(start, length) counts from 1.
[CmdletBinding(DefaultParameterSetName = 'Copy')]
param(
  [Parameter(Mandatory, ParameterSetName = 'Copy')] [ValidateSet('Word', 'Excel')] [string] $From,
  [Parameter(Mandatory, ParameterSetName = 'Close')] [switch] $Close
)
$ErrorActionPreference = 'Stop'

if ([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {
  throw 'the clipboard needs a single-threaded apartment: run this with pwsh, or with -STA'
}

if ($Close) {
  Get-CimInstance Win32_Process -Filter "Name = 'WINWORD.EXE' OR Name = 'EXCEL.EXE'" |
    Where-Object { $_.CommandLine -match '/Automation -Embedding' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
  return
}

if ($From -eq 'Word') {
  $word = New-Object -ComObject Word.Application
  $word.Visible = $false
  $document = $word.Documents.Add()
  $type = $word.Selection
  $type.TypeText('Plain start, ')
  $type.Font.Bold = 1; $type.TypeText('bold words'); $type.Font.Bold = 0
  $type.TypeText(', ')
  $type.Font.Italic = 1; $type.TypeText('italic'); $type.Font.Italic = 0
  $type.TypeText(', ')
  $type.Font.Underline = 1; $type.TypeText('underlined'); $type.Font.Underline = 0
  $type.TypeText(' and a ')
  $start = $type.Range.Start
  $type.TypeText('link')
  [void]$document.Hyperlinks.Add($document.Range($start, $type.Range.Start), 'https://example.com/page')
  $type.TypeText('.')
  $type.TypeParagraph()
  $type.Font.Color = 255   # red, which the paste must drop
  $type.TypeText('Red second paragraph.')
  $type.Font.Color = -16777216   # wdColorAutomatic
  $type.TypeParagraph()
  $type.Range.ListFormat.ApplyBulletDefault()
  $type.TypeText('First bullet'); $type.TypeParagraph()
  $type.TypeText('Second bullet'); $type.TypeParagraph()
  $type.Range.ListFormat.RemoveNumbers()
  $table = $document.Tables.Add($type.Range, 2, 2)
  $table.Cell(1, 1).Range.Text = 'H1'; $table.Cell(1, 2).Range.Text = 'H2'
  $table.Cell(2, 1).Range.Text = 'a'; $table.Cell(2, 2).Range.Text = 'b'
  $document.Content.Copy()
}
else {
  $excel = New-Object -ComObject Excel.Application
  $excel.Visible = $false
  $sheet = $excel.Workbooks.Add().Worksheets.Item(1)
  $sheet.Range('A1').Value2 = 'Name'; $sheet.Range('B1').Value2 = 'Amount'
  $sheet.Range('A2').Value2 = 'Apples'; $sheet.Range('B2').Value2 = 42
  $sheet.Range('A3').Value2 = 'Pears, ripe and cheap'; $sheet.Range('B3').Value2 = 7
  $sheet.Range('A1:B1').Font.Bold = $true
  $sheet.Range('A2').Font.Italic = $true
  $sheet.Range('B2').Font.Underline = 2   # xlUnderlineStyleSingle
  $sheet.Range('A3').Characters(8, 4).Font.Bold = $true     # "ripe"
  $sheet.Range('A3').Characters(13, 3).Font.Italic = $true  # "and"
  [void]$sheet.Range('A1:B3').Copy()
}

Add-Type -AssemblyName System.Windows.Forms
"on the clipboard: $([System.Windows.Forms.Clipboard]::GetDataObject().GetFormats() -join ', ')"
