# Enter and leaving the field finish an address as a comma does (docs/contacts.md), and a click on a
# pill's remove button is not leaving the field.
#
# WHY SHOWCASE. Nothing here sends: a composer is opened, typed in and discarded, which Close-Composer
# confirms. It needs an account to write from, which the showcase seeds.
#
# THE KEYS ARE REAL, and the pill is clicked, never invoked: whether focus leaves the input is the
# whole question, and an Invoke moves no focus.

Add-Type -AssemblyName System.Windows.Forms

$Address = 'bob@example.test'

function Open-Composer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) { throw 'no composer within 30s of pressing Compose' }
  # Typed before the editor is seeded, the keystrokes are wiped with it (ComposerUndo.Tests.ps1).
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while (-not (Find-UiaElement -AutomationId 'ToField').Current.HasKeyboardFocus) {
    if ($watch.Elapsed.TotalSeconds -ge 30) { throw 'the caret never reached To: the editor was not seeded within 30s' }
    Start-Sleep -Milliseconds 200
  }
}

function Send-Keys {
  param([Parameter(Mandatory)] [string] $Keys)
  Show-WinappWindow ([long] (Get-MailcalWindow).Current.NativeWindowHandle)
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds 400
}

# A character at a time, as ComposerUndo.Tests.ps1 types into To, and for its reason.
function Type-IntoTo {
  param([Parameter(Mandatory)] [string] $Text)
  $to = Find-UiaElement -AutomationId 'ToField'
  if (-not $to) { throw 'no ToField on screen' }
  Show-WinappWindow ([long] (Get-MailcalWindow).Current.NativeWindowHandle)
  $to.SetFocus()
  Start-Sleep -Milliseconds 300
  foreach ($character in $Text.ToCharArray()) {
    [System.Windows.Forms.SendKeys]::SendWait("$character")
    Start-Sleep -Milliseconds 40
  }
  Start-Sleep -Milliseconds 400
  $to
}

function Get-Pills {
  @(Find-UiaElements -Type Button | Where-Object { $_.Current.Name -like '*@example.test*' })
}

function Click-Element {
  param([Parameter(Mandatory)] [object] $Element)
  $bounds = $Element.Current.BoundingRectangle
  $point = @([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))
  Invoke-WinappPress -Point $point -Window ([long] (Get-MailcalWindow).Current.NativeWindowHandle)
  Start-Sleep -Milliseconds 600
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'Enter finishes the typed address and keeps the caret in the field'
      Body = {
        Open-Composer
        try {
          $to = Type-IntoTo $Address
          Send-Keys '{ENTER}'
          Assert-Equal 1 (Get-Pills).Count 'Enter must finish the typed address into a pill'
          Assert-Equal '' (Get-UiaText $to) 'the finished address must leave the input empty'
          Assert-True $to.Current.HasKeyboardFocus 'Enter must keep the caret in To for the next address'
          Send-Keys '^z'
          Assert-Equal $Address (Get-UiaText $to) 'Ctrl+Z after Enter must give the address back as text'
        }
        finally { $null = Close-Composer }
      }
    },
    @{
      Name = 'leaving the field finishes the typed address'
      Body = {
        Open-Composer
        try {
          $null = Type-IntoTo $Address
          Send-Keys '{TAB}'
          Assert-Equal 1 (Get-Pills).Count 'leaving To must finish the typed address into a pill'
        }
        finally { $null = Close-Composer }
      }
    },
    @{
      Name = "a click on a pill's remove button removes it, and finishes nothing"
      Body = {
        Open-Composer
        try {
          $to = Type-IntoTo "$Address,carol"
          Assert-Equal 1 (Get-Pills).Count 'a comma finishes an address into a pill'
          Click-Element (Get-Pills | Select-Object -First 1)
          Assert-Equal 0 (Get-Pills).Count 'the click must reach the remove button'
          Assert-Equal 'carol' (Get-UiaText $to) 'the address being typed must stay text'
        }
        finally { $null = Close-Composer }
      }
    }
  )
}
