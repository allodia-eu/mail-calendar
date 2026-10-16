# Undo and redo while writing (docs/undo.md): the recipient fields' own history, and the composer's
# right-click menus, whose Undo and Redo must reach the same history as the chords.
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. That assembly holds the step boundaries
# (RecipientHistoryTests) and which of WebView2's items the editor keeps (EditorMenu), and nothing
# about whether either is wired: whether the field hears the chord ahead of the TextBox, whether the
# TextBox's own history is kept out, whether the menus draw the field's and the bundle's Undo at
# all. Each of those passes every headless suite while broken.
#
# WHY SHOWCASE. Nothing here sends: a composer is opened, typed in and discarded, which Close-Composer
# confirms. It needs an account to write from, which the showcase seeds.
#
# THE KEYS ARE REAL. SendKeys, after SetFocus and with the window in front: a ValuePattern write
# would put the text in without a keystroke, and the history is made of keystrokes.
#
# THE MENU ITEMS ARE CLICKED, NEVER INVOKED. An item in a TextCommandBarFlyout's list answers UIA's
# Invoke without running its command, so an Invoke-based case would report a menu that does nothing
# as one that undid nothing.
#
# THE LABELS ARE THE PLATFORM'S, in the system's language, so no case finds an item by name: the
# field's Undo and Redo carry automation ids, and the body's are the first two items of WebView2's
# own menu, in the place WebView2 draws its own.

Add-Type -AssemblyName System.Windows.Forms

$Address = 'bob@example.test'

function Open-Composer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) { throw 'no composer within 30s of pressing Compose' }
  # The caret goes to To only once the editor has been seeded, and the seed replaces the body and
  # starts its history again: typed before it, a case's keystrokes are wiped and read as never sent.
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while (-not (Find-UiaElement -AutomationId 'ToField').Current.HasKeyboardFocus) {
    if ($watch.Elapsed.TotalSeconds -ge 30) { throw 'the caret never reached To: the editor was not seeded within 30s' }
    Start-Sleep -Milliseconds 200
  }
}

# Real keystrokes to whatever has focus, the window brought to the front first.
function Send-Keys {
  param([Parameter(Mandatory)] [string] $Keys)
  Show-WinappWindow ([long] (Get-MailcalWindow).Current.NativeWindowHandle)
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds 400
}

# Puts the caret in To and types -Text into it, a character at a time. Sent in one burst, the
# characters land in the input before it reports any of them, and it reports them as one change,
# which the field rightly files as one step, as it would a paste.
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

# The names of the remove buttons on the pills, one per finished recipient.
function Get-Pills {
  @(Find-UiaElements -Type Button | Where-Object { $_.Current.Name -like '*@example.test*' })
}

# A real click in the middle of -Element: see the header for why a menu item is never Invoked.
function Click-Element {
  param([Parameter(Mandatory)] [object] $Element)
  $bounds = $Element.Current.BoundingRectangle
  $point = @([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))
  Invoke-WinappPress -Point $point -Window ([long] (Get-MailcalWindow).Current.NativeWindowHandle)
  Start-Sleep -Milliseconds 600
}

# The WebView2 runtime's own top-level windows. The editor's page and its right-click menu are
# reached there, not under the app's window: the runtime is a process of its own.
function Get-WebViewWindows {
  [System.Windows.Automation.AutomationElement]::RootElement.FindAll($UiaChildren, $UiaAny) |
    Where-Object {
      $_.Current.ClassName -like 'Chrome_WidgetWin*' -and
      (Get-Process -Id $_.Current.ProcessId -ErrorAction SilentlyContinue).ProcessName -eq 'msedgewebview2'
    }
}

# The message body as UI Automation reads it: the editor element, named by its placeholder (the
# catalog's `editor_placeholder`, English in the showcase), whose TextPattern is the body's text.
# Waited for, because the page loads after the composer is on screen.
function Get-BodyGroup {
  $named = [System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::NameProperty, 'Write your message')
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt 15) {
    foreach ($window in Get-WebViewWindows) {
      $group = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $named)
      if ($group) { return $group }
    }
    Start-Sleep -Milliseconds 250
  }
  throw 'the editor exposes no body within 15s: has its page loaded?'
}

function Get-BodyText {
  $text = (Get-BodyGroup).GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
  $text.DocumentRange.GetText(-1)
}

# Clicks on the body's first line, which is the empty line a new message opens with.
function Click-Body {
  param([switch] $Right)
  $bounds = (Get-BodyGroup).Current.BoundingRectangle
  $point = @([int]($bounds.X + (ConvertTo-UiaPixels 24)), [int]($bounds.Y + (ConvertTo-UiaPixels 14)))
  Invoke-WinappPress -Point $point -Window ([long] (Get-MailcalWindow).Current.NativeWindowHandle) -Right:$Right
  Start-Sleep -Milliseconds 1200
}

# WebView2's right-click menu, a window of the runtime's own like the page.
function Get-BodyMenu {
  $item = [System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::MenuItem)
  @(Get-WebViewWindows | ForEach-Object { $_.FindAll([System.Windows.Automation.TreeScope]::Descendants, $item) } |
      Sort-Object { $_.Current.BoundingRectangle.Y })
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a finished address undoes to the text that was typed, and redoes to its pill'
      Body = {
        Open-Composer
        try {
          $to = Type-IntoTo "$Address,"
          Assert-Equal 1 (Get-Pills).Count 'a comma finishes an address into a pill'
          Send-Keys '^z'
          Assert-Equal $Address (Get-UiaText $to) `
            'Ctrl+Z after finishing an address must give back the text typed, not leave the input empty'
          Assert-Equal 0 (Get-Pills).Count `
            'Ctrl+Z after finishing an address must take its pill away, not finish it a second time'
          Send-Keys '^y'
          Assert-Equal 1 (Get-Pills).Count 'Ctrl+Y must put the pill back'
          Send-Keys '^z'
          Send-Keys '^+z'
          Assert-Equal 1 (Get-Pills).Count 'Shift+Ctrl+Z must redo as Ctrl+Y does'
          Assert-Equal '' (Get-UiaText $to) 'the redone pill leaves nothing half-typed in the input'
        }
        finally { $null = Close-Composer }
      }
    },
    @{
      Name = 'a removed recipient comes back with Ctrl+Z, from the field the caret was put back in'
      Body = {
        Open-Composer
        try {
          $to = Type-IntoTo "$Address, carol@example.test,"
          Assert-Equal 2 (Get-Pills).Count 'two finished addresses are two pills'
          Invoke-UiaElement (Get-Pills | Where-Object { $_.Current.Name -like "$Address*" } | Select-Object -First 1)
          Start-Sleep -Milliseconds 500
          Assert-Equal 1 (Get-Pills).Count 'the remove button took its pill away'
          Assert-True $to.Current.HasKeyboardFocus `
            'removing a pill must put the caret back in the field, where the chord that brings it back is heard'
          Send-Keys '^z'
          Assert-Equal 2 (Get-Pills).Count 'Ctrl+Z must bring the removed recipient back'
        }
        finally { $null = Close-Composer }
      }
    },
    @{
      Name = 'two quick undos are two steps, and both redo'
      Body = {
        Open-Composer
        try {
          $to = Type-IntoTo "$Address, carol@example.test,"
          Assert-Equal 2 (Get-Pills).Count 'two finished addresses are two pills'
          # One burst, so the second undo lands before the input reports the first.
          Send-Keys '^(zz)'
          Assert-True ((Get-Pills).Count -lt 2) 'two undos must take at least one address back'
          Send-Keys '^y'
          Send-Keys '^y'
          Assert-Equal 2 (Get-Pills).Count `
            'the field writing its own step into the input must not be filed as typing, which clears what there is to redo'
          Assert-Equal '' (Get-UiaText $to) 'both redone, nothing is left half-typed in the input'
        }
        finally { $null = Close-Composer }
      }
    },
    @{
      Name = "the To field's right-click menu undoes and redoes the field's own steps"
      Body = {
        Open-Composer
        try {
          $to = Type-IntoTo "$Address,"
          Send-Keys '+{F10}'
          $undo = Wait-UiaElement -AutomationId 'RecipientUndo' -TimeoutSec 5
          Assert-True ($null -ne $undo) "the field's menu offers no Undo of its own after an address was finished"
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'RecipientRedo')) `
            'the menu offers Redo with nothing undone, as a text box never does'
          Click-Element $undo
          Assert-Equal $Address (Get-UiaText $to) "the menu's Undo must take back the field's step, the address back to text"
          Send-Keys '+{F10}'
          $redo = Wait-UiaElement -AutomationId 'RecipientRedo' -TimeoutSec 5
          Assert-True ($null -ne $redo) "the field's menu offers no Redo after an undo"
          Click-Element $redo
          Assert-Equal 1 (Get-Pills).Count "the menu's Redo must put the pill back"
        }
        finally {
          Send-Keys '{ESC}'
          $null = Close-Composer
        }
      }
    },
    @{
      Name = "the body answers the chords, and its right-click menu's Undo and Redo are the same history"
      Body = {
        Open-Composer
        try {
          Click-Body
          Send-Keys 'hello'
          Assert-True ((Get-BodyText) -match 'hello') 'the typing never reached the body'
          Send-Keys '^z'
          Assert-True ((Get-BodyText) -notmatch 'hello') 'Ctrl+Z in the body must take back the typing'
          Send-Keys '^y'
          Assert-True ((Get-BodyText) -match 'hello') 'Ctrl+Y in the body must put the typing back'

          Click-Body -Right
          $menu = Get-BodyMenu
          Assert-True ($menu.Count -ge 2) 'no right-click menu over the body'
          # WebView2's own Redo is never on: the editor never runs the engine's undo, so that stack
          # has nothing to redo. One that is on here is the host's, reading the bundle.
          Assert-True $menu[0].Current.IsEnabled 'Undo must be on over typing the bundle can take back'
          Assert-True (-not $menu[1].Current.IsEnabled) 'Redo must be off with nothing undone'
          Click-Element $menu[0]
          Assert-True ((Get-BodyText) -notmatch 'hello') "the menu's Undo must take back the typing"
          Click-Body -Right
          $menu = Get-BodyMenu
          Assert-True (-not $menu[0].Current.IsEnabled) 'Undo must be off once everything is taken back'
          Assert-True $menu[1].Current.IsEnabled "Redo must be on after the menu's own Undo"
          Click-Element $menu[1]
          Assert-True ((Get-BodyText) -match 'hello') "the menu's Redo must put the typing back"
        }
        finally {
          Send-Keys '{ESC}'
          $null = Close-Composer
        }
      }
    },
    @{
      Name = 'Subject redoes on Shift+Ctrl+Z as well as Ctrl+Y'
      Body = {
        Open-Composer
        try {
          $subject = Find-UiaElement -AutomationId 'SubjectBox'
          Show-WinappWindow ([long] (Get-MailcalWindow).Current.NativeWindowHandle)
          $subject.SetFocus()
          Start-Sleep -Milliseconds 300
          Send-Keys 'hello'
          Send-Keys '^z'
          Assert-Equal '' (Get-UiaText $subject) 'Ctrl+Z in Subject must take back the typing'
          Send-Keys '^+z'
          Assert-Equal 'hello' (Get-UiaText $subject) 'Shift+Ctrl+Z in Subject must put it back, as it does in the body'
        }
        finally { $null = Close-Composer }
      }
    }
  )
}
