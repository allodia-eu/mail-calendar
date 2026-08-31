# Where the composer's caret opens, and that the suggestion list floats rather than displaces
# (docs/contacts.md §4).
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. Neither rule is reachable from a headless assembly.
# `FocusesBody`, the one predicate the contract binds every client to, lives on ComposeRequest,
# which builds its dialog title from L10n and so cannot be linked into a plain net10.0 project; and
# even if it could, the predicate answering correctly proves nothing about whether anything acted
# on it. A composer that computed the right answer and focused nothing would pass every headless
# suite in the repo, which is exactly the state this branch found the Apple client in.
#
# WHY HARNESS, AND NOT SHOWCASE. The list has to be LONG ENOUGH TO REACH THE EDITOR, §4 says so in
# as many words, because a list that stops above the web view passes while the bug is still there.
# The showcase people index holds two (both from sent mail), and a two-item list is shorter than
# the gap below every recipient field: measured at 200%, 172px of list against the 188px that
# separates even the LOWEST field from the editor's top, and more from the ones above it. It clears
# the web view entirely, so a showcase run of this file would report a confident green for the one
# thing it cannot see. The harness seeds ten-odd addresses, which fills the list to its 160epx cap
# and puts 130-odd pixels of it over the editor.
#
# Nothing here sends: composers are opened, read, and cancelled. Cancel discards without prompting
# (the "Discard draft?" question guards REPLACING an open composer, not closing one), so a case may
# type freely.

# How many suggestions a case needs before the overlap assertion means anything. Measured here,
# three already clears the gap between the field and the editor (258px against 188px) and two does
# not (172px); four is that with a margin, since row height moves with display scale and with a
# name long enough to wrap. Below it, "the list does not reach the editor" is the fixture being
# thin rather than the rule being broken, and the two must not report the same way.
$MinMatchesForOverlap = 4

# A token that matches most of the seeded address book. Substring, not prefix: every seeded address
# carries an 'e' somewhere.
$BroadToken = 'e'

<#
.SYNOPSIS
$true once the element with -AutomationId reports keyboard focus, polling until -TimeoutSec.
.DESCRIPTION
Polled, never slept: the caret is placed after the editor's seed snapshot, which waits on the
WebView2 coming up, so the delay is real and varies by machine. A fixed Start-Sleep either flakes
or wastes seconds on every case.

HasKeyboardFocus on the element itself, not AutomationElement::FocusedElement. When the caret is in
the message body, the focused element walks out to the WebView2's host site
(Microsoft.UI.Content.DesktopChildSiteBridge), an unnamed Pane with no AutomationId, which is
indistinguishable from any other hosted content. The WebView2 answers HasKeyboardFocus directly.
#>
function Wait-KeyboardFocus {
  param(
    [Parameter(Mandatory)] [string] $AutomationId,
    [int] $TimeoutSec = 25
  )
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $el = Find-UiaElement -AutomationId $AutomationId
    if ($el -and $el.Current.HasKeyboardFocus) { return $true }
    Start-Sleep -Milliseconds 250
  }
  return $false
}

# Whether the named element reports keyboard focus right now. $false when it is not on screen at
# all, which is the honest answer to "is the caret in it".
function Test-KeyboardFocus {
  param([Parameter(Mandatory)] [string] $AutomationId)
  $el = Find-UiaElement -AutomationId $AutomationId
  return ($null -ne $el) -and $el.Current.HasKeyboardFocus
}

# Top edge of an element, in physical pixels. Get-RenderedBounds rather than a raw
# BoundingRectangle read: a collapsed or scrolled-off element answers with infinities, and a
# comparison against one passes confidently (uia.ps1).
function Get-Top {
  param([Parameter(Mandatory)] [string] $AutomationId, [Parameter(Mandatory)] [string] $What)
  (Get-RenderedBounds -Element (Find-UiaElement -AutomationId $AutomationId) -What $What).Y
}

function Open-NewComposer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) {
    throw 'no composer within 30s of pressing Compose'
  }
}

function Open-Reply {
  $rows = Get-MailRows
  if ($rows.Count -eq 0) { throw 'the message list is empty, so there is nothing to reply to' }
  Invoke-UiaElement $rows[0] -SettleMs 2000
  $reply = Wait-UiaElement -AutomationId 'ReadingReply' -TimeoutSec 30
  if (-not $reply) { throw 'the reading pane never offered Reply, the message did not open' }
  Invoke-UiaElement $reply
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) {
    throw 'no composer within 30s of pressing Reply'
  }
}

# By id, not by the label: unlike a showcase suite this one runs in whatever language the developer
# has the app in, so matching 'Cancel' would make every case pass or fail by that.
function Close-Composer {
  $cancel = Find-UiaElement -AutomationId 'CancelButton' -Type Button
  if ($cancel) { Invoke-UiaElement $cancel }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
}

<#
.SYNOPSIS
Type -Token into the field with -AutomationId and return its open suggestion list, or $null.
.DESCRIPTION
SetFocus first, and deliberately: the field only offers suggestions for the FOCUSED field, so a
ValuePattern write alone would exercise a state the user can never be in.
#>
function Open-Suggestions {
  param(
    [Parameter(Mandatory)] [string] $AutomationId,
    [Parameter(Mandatory)] [string] $Token,
    [int] $TimeoutSec = 15
  )
  $field = Find-UiaElement -AutomationId $AutomationId
  if (-not $field) { throw "no $AutomationId on screen" }
  $field.SetFocus()
  Set-UiaText $field $Token
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $list = Find-UiaElement -AutomationId 'RecipientSuggestions'
    if ($list) { return $list }
    Start-Sleep -Milliseconds 250
  }
  return $null
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'a new message opens with the caret in To'
      Body = {
        Open-NewComposer
        try {
          Assert-True (Wait-KeyboardFocus -AutomationId 'ToField') `
            'the composer opened with To unfocused: writing a new message costs a click before a single character can be typed (docs/contacts.md §4)'
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'a reply opens with the caret in the body, not in To'
      Body = {
        Open-Reply
        try {
          Assert-True (Wait-KeyboardFocus -AutomationId 'Editor') `
            'a reply opened without the caret in the message body, its From/To/Subject are already filled in, so writing is the only thing left to do (docs/contacts.md §4)'
          # The other half of "exactly one of the two is focused". Without it, a client that focused
          # both in turn would pass the assertion above.
          Assert-True (-not (Test-KeyboardFocus -AutomationId 'ToField')) `
            'a reply put the caret in To as well as the body, §4 binds one predicate over (mode, To), so exactly one of the two is focused'
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'the suggestion list floats: nothing below it moves'
      Body = {
        Open-NewComposer
        try {
          $before = @{
            Subject = Get-Top -AutomationId 'SubjectBox' -What 'the Subject field'
            Editor  = Get-Top -AutomationId 'Editor' -What 'the message editor'
            Send    = Get-Top -AutomationId 'SendButton' -What 'the Send button'
          }
          $list = Open-Suggestions -AutomationId 'ToField' -Token $BroadToken
          if (-not $list) { throw "typing '$BroadToken' into To offered no suggestions at all, so this case measured nothing" }
          $after = @{
            Subject = Get-Top -AutomationId 'SubjectBox' -What 'the Subject field'
            Editor  = Get-Top -AutomationId 'Editor' -What 'the message editor'
            Send    = Get-Top -AutomationId 'SendButton' -What 'the Send button'
          }
          foreach ($part in 'Subject', 'Editor', 'Send') {
            Assert-Equal $before[$part] $after[$part] `
              "$part moved when the suggestion list opened: the list is taking layout space, so the whole form jumps down and back on every keystroke while the user is still typing the first recipient (docs/contacts.md §4)"
          }
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'the list is long enough to reach the editor, and is drawn over it'
      Body = {
        Open-NewComposer
        try {
          $list = Open-Suggestions -AutomationId 'ToField' -Token $BroadToken
          if (-not $list) { throw "typing '$BroadToken' into To offered no suggestions at all, so this case measured nothing" }
          # Asserted before the geometry, and separately: a short list clears the editor honestly,
          # so without this the case would report the fixture thinning out as the rule holding.
          $matches = @(Find-UiaElements -Type 'ListItem' -Root $list).Count
          Assert-GreaterThan ($MinMatchesForOverlap - 1) $matches `
            "only $matches suggestion(s) matched '$BroadToken', which is too short a list to reach the editor at all, this case can prove nothing against it, and §4 exists because a list that stops above the web view passes while the bug is still there. Reseed the harness (scripts/dev/harness.sh up)."

          $listBottom = (Get-RenderedBounds -Element $list -What 'the suggestion list').Bottom
          $editorTop = Get-Top -AutomationId 'Editor' -What 'the message editor'
          Assert-GreaterThan $editorTop $listBottom `
            'the suggestion list stops above the message editor, so this run never tested whether it can cover a hosted web view, the half of §4 that does not come free'
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'only the focused field offers suggestions'
      Body = {
        Open-NewComposer
        try {
          $toggle = Find-UiaElement -AutomationId 'CcBccToggle'
          if (-not $toggle) { throw 'no CcBccToggle, Cc cannot be reached, so focus cannot be moved off To' }
          Set-UiaToggle $toggle -On
          if (-not (Wait-UiaElement -AutomationId 'CcField' -TimeoutSec 10)) { throw 'Cc never appeared' }

          $list = Open-Suggestions -AutomationId 'ToField' -Token $BroadToken
          if (-not $list) { throw "typing '$BroadToken' into To offered no suggestions at all, so this case measured nothing" }

          (Find-UiaElement -AutomationId 'CcField').SetFocus()
          # Polled: the field closes its list on LosingFocus, which is raised on the UI thread.
          $watch = [Diagnostics.Stopwatch]::StartNew()
          while ($watch.Elapsed.TotalSeconds -lt 5 -and (Find-UiaElement -AutomationId 'RecipientSuggestions')) {
            Start-Sleep -Milliseconds 200
          }
          Assert-Equal $null (Find-UiaElement -AutomationId 'RecipientSuggestions') `
            "To's suggestion list is still on screen with the caret in Cc: harmless while the list sat in the layout, and covering a live field the moment it floats (docs/contacts.md §4)"
        }
        finally { Close-Composer }
      }
    }
  )
}
