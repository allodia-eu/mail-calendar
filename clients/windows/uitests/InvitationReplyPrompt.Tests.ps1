# "The organiser wasn't told", the bar the shell raises when the calendar server stored the RSVP
# and then reported it could not pass it on (RFC 6638 §3.2.9; docs/invitations.md, "A server can
# promise to send the reply, and then not send it").
#
# WHY THIS NEEDS A FAKE VERDICT. The state under test comes from the *server*, and it is the one
# no fixture can produce: every harness runs Stalwart, which delivers replies and reports nothing
# at all, while the server known to report `5.2` is somebody's production account. Until this
# suite existed the failure path was reached by editing invitations_fallback.rs by hand, once per
# platform. MAILCAL_FAKE_REPLY_DELIVERY (debug builds only) substitutes the verdict and nothing
# else, so everything below it is real: the core raises the question, signals
# Surface.InvitationReply, remembers what it is told and clears itself when answered.
#
# WHY THE CalDAV HALF. resolve_reply_delivery runs only on the *server* delivery route, which is
# CalDAV auto-schedule, so MAILCAL_DEV_ACCOUNT=stalwart-imap, not the runner's JMAP default. It
# also puts this suite on its own engine store (dev-imap), away from the other harness suites'.
#
# WHY IT DECLINES RATHER THAN ACCEPTS. The RSVP is real and lands on the shared harness server, so
# which invitation this answers is a fixture decision, not a detail:
#   - "Quarterly planning" is where Attendees.Tests reads alice as the one who has NOT answered.
#     Answering it there would make that suite pass or fail by what ran before it.
#   - "Weekend walk" is the free-day fixture InvitationPreview.Tests needs to stay empty. A
#     DECLINED event is hidden from every calendar surface (docs/invitations.md), so declining
#     leaves that day as empty as it found it, where ACCEPTING puts an event on it, which is
#     exactly how a hand-run of this check broke that suite once.
#
# NOTHING BELOW HARDCODES ENGLISH. The app's language on a harness run is the developer's own
# preference, so every string is matched against the shared catalog (messages/<locale>.json).
# That is a stronger assertion than a literal anyway: it pins that the control is bound to the
# RIGHT KEY, in whatever language this machine happens to run.

$Invitation = 'Weekend walk'          # declined, so the free-day fixture stays free
$Organizer = 'bob@test.local'         # from the seed, not the catalog
$FakeStatus = '5.2'                   # what the core is told to pretend the server reported

$CatalogDir = Join-Path $PSScriptRoot '../../../messages'
# Where the core keeps this run's preferences, AppPaths.PrefsDir for MAILCAL_DEV_ACCOUNT=stalwart-imap.
$PrefsFile = Join-Path $env:LOCALAPPDATA 'Allodia/MailCalendar/dev-imap/preferences.toml'

# ---------------------------------------------------------------------------------------------
# Catalog + element helpers
# ---------------------------------------------------------------------------------------------

<#
.SYNOPSIS
Every catalog locale's value for one message key.
.DESCRIPTION
The whole catalog rather than one locale on purpose: resolving "which language is this app in"
means re-deriving LanguageStore's fallback chain here, and getting that wrong turns a correct app
into a red suite. Matching against the set answers the question the test actually asks, is this
control showing THIS message, without caring which language it landed in.
#>
function Get-CatalogValues {
  param([Parameter(Mandatory)] [string] $Key)
  $values = @()
  foreach ($file in Get-ChildItem -Path $CatalogDir -Filter '*.json') {
    $json = Get-Content -LiteralPath $file.FullName -Raw -Encoding utf8 | ConvertFrom-Json
    if ($json.PSObject.Properties.Name -contains $Key) { $values += $json.$Key }
  }
  if (-not $values) { throw "no catalog locale defines '$Key', the key was renamed or removed" }
  $values
}

<#
.SYNOPSIS
The one Button under $Scope whose name is this key in any catalog locale, or $null.
#>
function Find-CatalogButton {
  param(
    [Parameter(Mandatory)] [string] $Key,
    [object] $Scope
  )
  $wanted = Get-CatalogValues $Key
  if (-not $Scope) { $Scope = Get-MailcalWindow }
  Get-UiaTree $Scope |
    Where-Object {
      $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
      $wanted -contains $_.Current.Name
    } | Select-Object -First 1
}

<#
.SYNOPSIS
The reply-undelivered InfoBar, or $null.
.DESCRIPTION
Scoped by ClassName and then by its own title: MainWindow hosts several InfoBars (the time-zone
change, the connection banner), and the invitation card below carries a CheckBox of its own
("Email the organiser"). A bare sweep for "the first CheckBox in the window" would read that one
and report a confident green about a control in a different feature, uia.ps1 trap 2, one layer up.
#>
function Get-ReplyBar {
  $titles = Get-CatalogValues 'invitation_reply_undelivered_title'
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'Microsoft.UI.Xaml.Controls.InfoBar')
  foreach ($bar in (Get-MailcalWindow).FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)) {
    if (Get-UiaTree $bar | Where-Object { $titles -contains $_.Current.Name }) { return $bar }
  }
  $null
}

<#
.SYNOPSIS
Poll for the bar (the RSVP is a real round trip to the harness), or throw saying what it means.
#>
function Wait-ReplyBar {
  param([int] $TimeoutSec = 60)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $bar = Get-ReplyBar
    if ($bar) { return $bar }
    Start-Sleep -Milliseconds 300
  }
  throw "no reply-undelivered bar within ${TimeoutSec}s. Check in this order: the answer never went out at all (a stale client store leaves the invitation with no card and nothing to press, 'scripts/dev/harness.sh reset'); this is not a DEBUG build, so MAILCAL_FAKE_REPLY_DELIVERY is compiled out (app.log carries a warning whenever it IS in force); Surface.InvitationReply no longer reaches the projection; the InfoBar's IsOpen binding is not wired."
}

<#
.SYNOPSIS
Wait for the bar to go away, then report whether it did.
#>
function Test-ReplyBarClosed {
  param([int] $TimeoutSec = 30)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec -and (Get-ReplyBar)) { Start-Sleep -Milliseconds 300 }
  -not (Get-ReplyBar)
}

<#
.SYNOPSIS
Open the invitation and answer it.
#>
function Invoke-Rsvp {
  param([Parameter(Mandatory)] [string] $Key)
  Invoke-UiaElement (Get-MailRowByTitle $Invitation)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt 45) {
    $button = Find-CatalogButton $Key
    if ($button) { Invoke-UiaElement $button; return }
    Start-Sleep -Milliseconds 300
  }
  throw "'$Invitation' produced no invitation card, so there was nothing to answer. The usual cause is not the RSVP gate but a stale client store: a re-bootstrapped Stalwart reuses its ids for a different set of messages, so this one opens somebody else's body, with no calendar part in it. 'scripts/dev/harness.sh reset' clears both halves."
}

<#
.SYNOPSIS
Forget this account's standing answer to the prompt.
.DESCRIPTION
Called BEFORE the first RSVP rather than after the last, so the suite heals whatever the previous
run left behind, including a run that died between ticking the box and getting here. A remembered
"never" makes the core answer for the user and raise nothing, which would turn every case below
into a silent skip dressed as a failure to find the bar.

Editing the file under the running app is safe because the core re-reads it on each RSVP and
rewrites it whole from that read (`load_preferences` → `set_reply_fallback` → `save_preferences`),
so there is no in-memory copy to disagree with this.
#>
function Clear-RememberedChoice {
  if (-not (Test-Path -LiteralPath $PrefsFile)) { return }
  $kept = @()
  $inTable = $false
  foreach ($line in Get-Content -LiteralPath $PrefsFile) {
    if ($line -match '^\s*\[') { $inTable = $line -match '^\s*\[invitation_reply_fallback\]\s*$' }
    if (-not $inTable) { $kept += $line }
  }
  Set-Content -LiteralPath $PrefsFile -Value $kept
}

<#
.SYNOPSIS
Every readable string inside the bar (its title, its body, and whatever else it draws).
.DESCRIPTION
Deliberately not "the second Text node": an InfoBar also exposes its severity glyph as text
("Warning icon"), so a positional pick reads the icon and reports that the message is missing.
Collecting them all and asking whether the expected one is among them survives the chrome
changing, and prints what WAS there when it fails.
#>
function Get-BarTexts {
  param([Parameter(Mandatory)] [object] $Bar)
  Get-UiaTree $Bar |
    Where-Object {
      $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and $_.Current.Name
    } | ForEach-Object { $_.Current.Name }
}

<#
.SYNOPSIS
True when the bar shows the catalog's undelivered-body message with $Summary and $Organizer in its slots.
.DESCRIPTION
Built from the template rather than compared to a literal, so this pins THE WHOLE SENTENCE, the
order its clauses come in included. That is what makes rule 1 ("the RSVP worked, and the prompt
says so first") a check rather than a comment: swapping the body for a "couldn't send…" string, or
for a generic error message, matches no locale's template.
#>
function Test-BarShowsBody {
  param(
    [Parameter(Mandatory)] [object] $Bar,
    [Parameter(Mandatory)] [string] $Summary,
    [Parameter(Mandatory)] [string] $Organizer
  )
  $shown = @(Get-BarTexts $Bar)
  foreach ($template in Get-CatalogValues 'invitation_reply_undelivered_body') {
    if ($shown -contains $template.Replace('{summary}', $Summary).Replace('{organizer}', $Organizer)) { return $true }
  }
  $false
}

# ---------------------------------------------------------------------------------------------

$Suite = @{
  Dataset = 'harness'
  # The CalDAV half (resolve_reply_delivery runs on the server route only), plus the verdict no
  # server here will produce. Both are debug-only; the runner clears them after this suite.
  Env     = @{
    MAILCAL_DEV_ACCOUNT         = 'stalwart-imap'
    MAILCAL_FAKE_REPLY_DELIVERY = "failed:$FakeStatus"
  }
  Cases   = @(
    @{
      Name = 'a reported failure raises the bar, naming the meeting and the person who would be emailed'
      Body = {
        Clear-RememberedChoice
        Invoke-Rsvp 'a11y_invitation_decline'
        $bar = Wait-ReplyBar

        # Rules 1 and 2 together (docs/invitations.md): the sentence is the catalog's own, in the
        # order the catalog wrote it, the answer is saved FIRST, with the meeting and the
        # organiser's ADDRESS in its slots. A person consenting to send mail as themselves is
        # entitled to see the recipient, so "the organiser" would not do.
        Assert-True (Test-BarShowsBody -Bar $bar -Summary $Invitation -Organizer $Organizer) `
          "the bar must show invitation_reply_undelivered_body with the meeting and the organizer in its slots, it shows: $((Get-BarTexts $bar) -join ' | ')"
      }
    },
    @{
      Name = 'the RFC status code is nowhere on screen'
      Body = {
        # Rule 3, and the assertion here that can only pass for the right reason: this suite CHOSE
        # the token, so finding it would mean the code leaked out of the diagnostics log and into
        # a modal, where "5.2" explains nothing to the person reading it.
        $bar = Wait-ReplyBar
        $shown = @(Get-UiaTree $bar | Where-Object { $_.Current.Name -like "*$FakeStatus*" })
        Assert-Equal 0 $shown.Count `
          "the status code rides the prompt for the log, never for the user, found it in: $(($shown | ForEach-Object { $_.Current.Name }) -join ' | ')"
      }
    },
    @{
      Name = 'the remember tick is present, labelled from the catalog, and starts clear'
      Body = {
        # Never pre-ticked: ticked beside either button it sets a STANDING choice for the account,
        # and beside "Send the email" that is standing permission to send mail as the user.
        $bar = Wait-ReplyBar
        $box = Get-UiaTree $bar | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::CheckBox } | Select-Object -First 1
        Assert-True ([bool]$box) 'the bar must offer the remembered-choice tick, or a server that fails every reply asks at every meeting'
        Assert-True ((Get-CatalogValues 'invitation_reply_undelivered_remember') -contains $box.Current.Name) `
          "the tick must be labelled from invitation_reply_undelivered_remember, got: $($box.Current.Name)"
        Assert-Equal $false (Get-UiaToggle $box) 'consent is never pre-given'
      }
    },
    @{
      Name = 'the bar offers exactly two ways out, and neither of them closes it unanswered'
      Body = {
        # Nothing may dismiss the question without answering it, or the core goes on holding a
        # prompt the user can no longer see. IsClosable="False" is what enforces that, and it is
        # one property nothing else watches.
        $bar = Wait-ReplyBar
        $buttons = @(Get-UiaTree $bar | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button })
        $names = @($buttons | ForEach-Object { $_.Current.Name })
        Assert-Equal 2 $buttons.Count `
          "the bar must offer the two answers and no third way out (an InfoBar close button would be a third), found: $($names -join ' | ')"
        Assert-True ([bool](Find-CatalogButton 'invitation_reply_undelivered_send' -Scope $bar)) `
          "one button must be invitation_reply_undelivered_send, found: $($names -join ' | ')"
        Assert-True ([bool](Find-CatalogButton 'invitation_reply_undelivered_dismiss' -Scope $bar)) `
          "one button must be invitation_reply_undelivered_dismiss, found: $($names -join ' | ')"
      }
    },
    @{
      Name = 'answering closes it, and a remembered no stops the next meeting asking'
      Body = {
        # Two rules in one pass, because the second needs the first to have happened IN THIS
        # PROCESS. Rule 4's easy half is the tick beside "Send"; its hard half is the tick beside
        # "Don't send", a standing NO, whose only symptom when dropped is being asked forever on
        # exactly the server the setting exists for.
        $bar = Wait-ReplyBar
        $box = Get-UiaTree $bar | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::CheckBox } | Select-Object -First 1
        Set-UiaToggle $box -On
        Invoke-UiaElement (Find-CatalogButton 'invitation_reply_undelivered_dismiss' -Scope $bar)

        Assert-True (Test-ReplyBarClosed) `
          'the core clears the question as it is answered and signals the surface; a bar still standing means the client is not mirroring that'
        Assert-True ([bool](Get-Content -LiteralPath $PrefsFile | Select-String -SimpleMatch 'invitation_reply_fallback' -Quiet)) `
          'the tick beside "Don''t send" must be stored as this account''s standing answer'

        Invoke-Rsvp 'a11y_invitation_decline'
        Start-Sleep -Seconds 6
        Assert-True (-not (Get-ReplyBar)) 'a remembered "no" must stop the next meeting asking'
      }
    },
    @{
      Name = 'the tick does not carry over to the next meeting'
      Body = {
        # An InfoBar's content is NOT re-created when IsOpen goes false and back, so a tick left
        # standing from the meeting before would silently set a standing choice the user made once
        # and was never shown again. The code-behind clears it by hand; this is the only machine
        # that can tell whether that line is still there, and it has to run in the same process
        # that ticked it, which is why it sits after the case above rather than in a fresh launch.
        Clear-RememberedChoice
        Invoke-Rsvp 'a11y_invitation_decline'
        $bar = Wait-ReplyBar
        $box = Get-UiaTree $bar | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::CheckBox } | Select-Object -First 1
        Assert-Equal $false (Get-UiaToggle $box) `
          'this prompt is a different meeting; the tick from the last one may not still be standing'

        # Leave the account as this suite found it: unticked, so nothing is remembered, and the
        # next run starts from the same place.
        Invoke-UiaElement (Find-CatalogButton 'invitation_reply_undelivered_dismiss' -Scope $bar)
        Assert-True (Test-ReplyBarClosed) 'and it still closes'
        Clear-RememberedChoice
      }
    }
  )
}
