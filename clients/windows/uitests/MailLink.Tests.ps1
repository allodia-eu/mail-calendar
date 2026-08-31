# A `mailto:` link opens the composer, pre-filled with what the link named, and with nothing else
# (docs/composer-security.md, Gate 12).
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. That assembly reaches the gate that decides an
# activation IS a mail link, and the core's own Rust suite reaches the decode. Neither can see the
# part in between: that the decoded fields are actually assigned to the composer's controls, on the
# real activation path, in a running window. That seam is where a mail link would silently arrive
# as a blank composer, with the link "working" and nothing in it.
#
# WHY IT LAUNCHES A SECOND PROCESS. That is not a trick, it is the real path. The app is
# single-instanced, so a link clicked while it is running is redirected into the live instance
# exactly as this does it (Program.OnActivated). Driving the composer directly would prove nothing
# about activation, which is the half that is new.
#
# WHY SHOWCASE. Nothing here sends, moves, or deletes anything: the composer is opened, read, and
# cancelled. What it does need is an account to send from, a composer with no From is refused,
# and the showcase seeds two, deterministically.

$Link = 'mailto:bob@example.com?subject=Lunch%20on%20Friday&cc=carol@example.com&bcc=dave@example.com'
# The same link plus the header a link is never allowed to set. `from` is the one that matters most:
# honoured, it would let any web page decide who a message appears to come from.
$SpoofedLink = 'mailto:bob@example.com?from=spoof@evil.test&reply-to=spoof@evil.test&subject=Hi'

# Hands a link to the running app the way the OS does, and waits for the composer it must open.
function Open-MailLink {
  param([Parameter(Mandatory)] [string] $Uri)
  $exe = (Get-Process Mailcal -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).Path
  if (-not $exe) { throw 'the client is not running, nothing to hand a mail link to' }
  Start-Process -FilePath $exe -ArgumentList $Uri | Out-Null
  $send = Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30
  if (-not $send) {
    throw "no composer within 30s of the mail link, the activation never reached the shell, or it was dropped before opening one"
  }
}

# The recipients the open composer is holding, each with the Y it is drawn at, top-first.
#
# Scoped to the pills' REMOVE buttons rather than to a container: `ComposerHost` is a ContentControl
# and `ComposerView` a UserControl, and neither gets an automation peer, a lookup by either id
# finds nothing and reads as "the composer never opened". A remove button, on the other hand, exists
# nowhere but a recipient field, and names the address it would remove.
#
# The Y is what tells To from Cc from Bcc. There is no other way to: the three fields are one
# control used three times, so every pill in all three looks identical to UIA. They are stacked in
# that order (ComposerView.xaml rows 2/3/4), so top-to-bottom IS the field order, which is what
# makes "the Bcc landed in the Bcc row" something this can fail on rather than assume.
function Get-ComposerRecipients {
  Get-UiaTree (Get-MailcalWindow) |
    Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button } |
    Where-Object { $_.Current.Name -match '^(?<address>[^\s@]+@[^\s@]+)\s' } |
    ForEach-Object {
      [pscustomobject]@{
        Address = $Matches.address
        # Straight off the element, not through Get-RenderedBounds: an offscreen pill would report
        # infinities, and here that is a failure to surface, not a value to sanitise away.
        Y       = $_.Current.BoundingRectangle.Y
      }
    } | Sort-Object Y
}

# Every piece of text anywhere in the window, for asking what must NOT be on screen.
#
# Names AND text-box values, because the two carry different halves and the missing half is the
# dangerous one: a TextBox's Name is its header ("Subject"), never its content, so a walk over names
# alone reads a composer whose subject is a spoofed address as clean. Verified by making a spoof
# land in the Subject field and watching this go red.
function Get-WindowText {
  Get-UiaTree (Get-MailcalWindow) | ForEach-Object {
    if ($_.Current.Name) { $_.Current.Name }
    # Not every element exposes ValuePattern, and asking one that doesn't throws.
    try { $value = Get-UiaText $_; if ($value) { $value } } catch { }
  }
}

function Close-Composer {
  $cancel = Find-UiaElement -Name 'Cancel' -Type Button
  if ($cancel) { Invoke-UiaElement $cancel }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a mail link opens the composer with every field it named'
      Body = {
        Open-MailLink -Uri $Link
        try {
          $recipients = @(Get-ComposerRecipients)
          # In field order, top-first. Cc and Bcc matter twice over: they must arrive, AND they must
          # be on screen, a recipient the user cannot see before pressing Send is one they cannot
          # remove, which is why Gate 12 makes a pre-filled Bcc a visibility rule and not a nicety.
          Assert-Equal 'bob@example.com, carol@example.com, dave@example.com' `
            (($recipients | ForEach-Object { $_.Address }) -join ', ') `
            'the composer opened with the wrong recipients, or in the wrong fields, top-to-bottom is To, Cc, Bcc'
          Assert-Equal 'Lunch on Friday' (Get-UiaText (Find-UiaElement -AutomationId 'SubjectBox')) `
            'the subject arrived wrong, a %20 decoded by anything but the shared core is the usual cause'
        }
        finally { Close-Composer }
      }
    },
    @{
      Name = 'a link cannot dictate who the message comes from'
      Body = {
        # The allowlist is enforced in the shared core, so it holds on every platform, but the
        # client is where it would be undone, by putting the URI through some other decoder on the
        # way to the fields. Asserted on what the user can actually see.
        Open-MailLink -Uri $SpoofedLink
        try {
          Assert-True (-not (@(Get-WindowText) -match 'spoof@evil\.test')) `
            'a header the link was never allowed to set reached the composer: `from` decides who a message appears to come from, and no web page may choose it'
          Assert-Equal 'bob@example.com' `
            ((@(Get-ComposerRecipients) | ForEach-Object { $_.Address }) -join ', ') `
            'the honoured half of the link was dropped along with the spoofed half'
        }
        finally { Close-Composer }
      }
    }
  )
}
