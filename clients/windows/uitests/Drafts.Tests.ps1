# Keeping the message being composed on the server (docs/drafts.md): Save as draft, the idle save,
# leaving a composer, a Drafts row opening back into its composer, and the two ways a stored copy
# leaves, Discard and an accepted send.
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. Every rule below is wiring in ComposerView.Drafts.cs,
# MainWindow.Drafts.cs and the list's click handlers, all of which link WinUI. The idle save is the
# sharpest edge: the body is a WebView2 with no channel back, so the host samples the editor's own
# change count, and a sampler that never ran would leave every headless suite green while no draft
# typed in the body ever reached the server.
#
# WHY THE HARNESS. A draft is a mail write, and the showcase engine performs none, so a save there
# proves only that something was dispatched. Every case asserts on the Drafts folder as the server
# lists it after a sync.
#
# Each case writes a draft under a subject of its own and takes it off the server again, through the
# composer's Discard, so the suite leaves Drafts as it found it. The send case leaves one message in
# Sent, as every sending suite does.

$CatalogDir = Join-Path $PSScriptRoot '../../../messages'
$DraftAccount = 'alice@test.local'
$DraftRecipient = 'bob@test.local'
# Per run, so a copy left behind by an interrupted run is never mistaken for this run's.
$DraftRun = Get-Date -Format 'HHmmss'

. (Join-Path $PSScriptRoot 'appwindows.ps1')

$DraftCatalog = Get-Content -LiteralPath (Join-Path $CatalogDir 'en.json') -Raw -Encoding utf8 | ConvertFrom-Json

# Opens one of the account's folders, by the row the pane draws for it under the account.
function Open-DraftAccountFolder {
  param([Parameter(Mandatory)] [string] $Folder)
  $accountRow = Find-UiaElement -Root (Get-MailcalWindow) -Name $DraftAccount -Type 'ListItem'
  if (-not $accountRow) { throw "the pane has no row for $DraftAccount" }
  $row = Find-UiaElement -Root $accountRow -Name $Folder -Type 'ListItem'
  if (-not $row) { throw "$DraftAccount shows no $Folder row; is its tree open?" }
  Invoke-UiaElement -Element $row | Out-Null
  [void](Wait-MailRowCount)
}

# Whether the open list shows a row titled $Subject. A bare `-like` on every row, rather than
# Get-MailRowByTitle, because that one throws on a miss and half of these waits want the miss.
function Test-DraftRow {
  param([Parameter(Mandatory)] [string] $Subject)
  foreach ($row in @(Get-MailRows)) {
    if (Get-UiaTree $row | Where-Object { $_.Current.Name -like $Subject }) { return $true }
  }
  $false
}

# Opens Drafts, asks the server for it, and waits until $Subject is listed (or, with -Absent, is
# not). The Sync matters for -Absent: without it a removal the list has not heard of yet reads as
# the row still being there, and a present row could be the one the list already had.
function Wait-DraftRow {
  param([Parameter(Mandatory)] [string] $Subject, [switch] $Absent, [int] $TimeoutSec = 30)
  Open-DraftAccountFolder -Folder 'Drafts'
  $sync = Find-UiaElement -AutomationId 'SelectionSync' -Type 'Button'
  if ($sync) { Invoke-UiaElement -Element $sync | Out-Null }
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $shown = $false
    try { $shown = Test-DraftRow -Subject $Subject } catch { }
    if ($shown -ne [bool]$Absent) { return $true }
    Start-Sleep -Milliseconds 250
  }
  $false
}

# The composer's draft hint, or '' when it draws none.
function Get-DraftHint {
  $hint = Find-UiaElement -AutomationId 'DraftHint'
  if ($hint) { $hint.Current.Name } else { '' }
}

function Wait-DraftSaved {
  param([int] $TimeoutSec = 15)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    if ((Get-DraftHint) -eq $DraftCatalog.compose_draft_saved) { return $true }
    Start-Sleep -Milliseconds 250
  }
  $false
}

function Open-NewDraftComposer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) { throw 'no composer within 30s of pressing Compose' }
}

# Writes a new message addressed and titled, and stores it with Save as draft. Leaves the composer
# open.
function New-SavedDraft {
  param([Parameter(Mandatory)] [string] $Subject)
  Open-NewDraftComposer
  Set-UiaText (Find-UiaElement -AutomationId 'ToField') $DraftRecipient
  Set-UiaText (Find-UiaElement -AutomationId 'SubjectBox') $Subject
  $save = Find-UiaElement -AutomationId 'SaveDraftButton' -Type 'Button'
  if (-not $save) { throw 'the composer draws no Save as draft button' }
  Invoke-UiaElement $save
  if (-not (Wait-DraftSaved)) {
    throw "Save as draft never reported the draft saved; the hint reads '$(Get-DraftHint)'"
  }
}

# Whether "Discard draft?" is up: the only dialog these cases raise, so its primary button is it.
function Test-DiscardQuestion {
  Wait-UiaQuiet -CapMs 800
  $null -ne (Find-UiaElement -AutomationId 'PrimaryButton' -Type 'Button')
}

# Leaves the open composer the way a person does, by opening a message from the Inbox. Leaving keeps
# the draft and asks nothing (docs/drafts.md, "Leaving a composer"), so a question here is a failure.
function Close-DraftComposer {
  if (-not (Find-UiaElement -AutomationId 'SendButton')) { return }
  Open-DraftAccountFolder -Folder 'Inbox'
  Invoke-UiaElement (@(Get-MailRows)[0]) | Out-Null
  if (Test-DiscardQuestion) { throw 'leaving the composer asked "Discard draft?"; leaving keeps the draft unasked' }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
}

# Opens the Drafts row titled $Subject, by however many presses, and waits for the composer.
function Open-StoredDraft {
  param([Parameter(Mandatory)] [string] $Subject)
  if (-not (Wait-DraftRow -Subject $Subject)) { throw "Drafts does not list '$Subject'" }
  Invoke-UiaElement (Get-MailRowByTitle $Subject) | Out-Null
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) {
    throw "the Drafts row '$Subject' opened no composer within 30s"
  }
}

# Throws the open composer's draft away with its Discard, answering "Discard draft?" when it asks.
# Answers whether it asked. A no-op with no composer up, so cleanup can call it blind.
function Remove-OpenDraft {
  $discard = Find-UiaElement -AutomationId 'DiscardButton' -Type 'Button'
  if (-not $discard) { return $false }
  Invoke-UiaElement $discard
  $asked = Test-DiscardQuestion
  if ($asked) { Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type 'Button') }
  Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
  $asked
}

# Takes $Subject off the server however a case left it, so the next run starts from the same
# Drafts. Best effort: a case that already removed it has nothing to clean.
function Clear-StoredDraft {
  param([Parameter(Mandatory)] [string] $Subject)
  Close-ExtraWindows
  # Discarded rather than left: leaving would put whatever a failed case typed into Drafts.
  Remove-OpenDraft | Out-Null
  if (-not (Wait-DraftRow -Subject $Subject -TimeoutSec 5)) { return }
  Open-StoredDraft -Subject $Subject
  Remove-OpenDraft | Out-Null
}

# Stores a draft as another client would, straight into the account's Drafts over JMAP: an HTML
# body whose only picture is a `cid:` part, a solid red square, ahead of one line of text. The
# harness advertises its JMAP URLs under a hostname this machine does not resolve, so only their
# paths are used.
function Add-ForeignDraft {
  param([Parameter(Mandatory)] [string] $Subject)
  $base = 'http://127.0.0.1:28080'
  $auth = @{ Authorization = 'Basic ' + [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("${DraftAccount}:harness-alice-pw")) }
  $session = Invoke-RestMethod "$base/jmap/session" -Headers $auth
  $account = $session.primaryAccounts.'urn:ietf:params:jmap:mail'
  $local = { param($url) ($url -replace '^https?://[^/]+', $base) -replace '\{accountId\}', $account }
  $message = @(
    "From: $DraftAccount", "To: $DraftRecipient", "Subject: $Subject",
    "Date: $([DateTime]::UtcNow.ToString('r'))", "Message-ID: <$([guid]::NewGuid())@test.local>",
    'MIME-Version: 1.0', 'Content-Type: multipart/related; boundary="part"', '',
    '--part', 'Content-Type: text/html; charset=utf-8', '',
    '<p><img src="cid:square@test.local" alt="" width="40" height="40"></p><p>Written elsewhere</p>',
    '--part', 'Content-Type: image/png', 'Content-ID: <square@test.local>', 'Content-Transfer-Encoding: base64', '',
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
    '--part--', ''
  ) -join "`r`n"
  $blob = Invoke-RestMethod (& $local $session.uploadUrl) -Method Post -Headers $auth -ContentType 'message/rfc822' `
    -Body ([Text.Encoding]::UTF8.GetBytes($message))
  $call = { param($method, $arguments)
    $body = @{ using = @('urn:ietf:params:jmap:core', 'urn:ietf:params:jmap:mail'); methodCalls = @(, @($method, $arguments, 'c')) }
    (Invoke-RestMethod (& $local $session.apiUrl) -Method Post -Headers $auth -ContentType 'application/json' `
      -Body ($body | ConvertTo-Json -Depth 10)).methodResponses[0][1]
  }
  $drafts = (& $call 'Mailbox/query' @{ accountId = $account; filter = @{ role = 'drafts' } }).ids[0]
  $imported = & $call 'Email/import' @{ accountId = $account; emails = @{ d = @{
    blobId = $blob.blobId; mailboxIds = @{ $drafts = $true }; keywords = @{ '$draft' = $true; '$seen' = $true } } } }
  if (-not $imported.created.d) { throw "the harness refused the draft: $($imported | ConvertTo-Json -Depth 5 -Compress)" }
}

# How many pixels inside the open composer's editor are the fixture's red.
function Get-EditorRedPixels {
  $editor = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'Editor') -What 'the message editor'
  $bmp = New-Object System.Drawing.Bitmap ([int]$editor.Width), ([int]$editor.Height)
  try {
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try { $g.CopyFromScreen([int]$editor.X, [int]$editor.Y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
    $red = 0
    for ($x = 0; $x -lt $bmp.Width; $x += 2) {
      for ($y = 0; $y -lt $bmp.Height; $y += 2) {
        $c = $bmp.GetPixel($x, $y)
        if ($c.R -gt 200 -and $c.G -lt 60 -and $c.B -lt 60) { $red++ }
      }
    }
    $red
  }
  finally { $bmp.Dispose() }
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'Save as draft stores the message in Drafts, and leaving keeps it there'
      Body = {
        $subject = "Draft suite $DraftRun save"
        try {
          New-SavedDraft -Subject $subject
          Close-DraftComposer
          Assert-True (Wait-DraftRow -Subject $subject) `
            'the draft Save as draft reported saved is not in Drafts after leaving the composer: either the save never reached the server, or leaving took it away'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'leaving a composer that was written in saves it, and asks nothing'
      Body = {
        # Never saved by hand and left at once, well inside the idle interval, so only the leave
        # can have put it on the server.
        $subject = "Draft suite $DraftRun leave"
        try {
          Open-NewDraftComposer
          Set-UiaText (Find-UiaElement -AutomationId 'ToField') $DraftRecipient
          Set-UiaText (Find-UiaElement -AutomationId 'SubjectBox') $subject
          Close-DraftComposer
          Assert-True (Wait-DraftRow -Subject $subject) `
            'a composer left by opening another message is not in Drafts: leaving must save what was written'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'Discard over a composer nothing was written in asks nothing'
      Body = {
        Open-NewDraftComposer
        Assert-True (-not (Remove-OpenDraft)) `
          'Discard over an untouched composer asked "Discard draft?"; with nothing written and nothing stored there is nothing to lose'
        Assert-True ($null -eq (Find-UiaElement -AutomationId 'SendButton')) `
          'Discard over an untouched composer left it open'
      }
    },
    @{
      Name = 'a Drafts row opens back into its composer, holding what was saved'
      Body = {
        $subject = "Draft suite $DraftRun resume"
        try {
          New-SavedDraft -Subject $subject
          Close-DraftComposer
          Open-StoredDraft -Subject $subject
          Assert-Equal $subject (Get-UiaText (Find-UiaElement -AutomationId 'SubjectBox')) `
            'the resumed composer does not hold the subject the draft was saved with'
          # A pill's accessible name is "<address>, Remove recipient", in the app's language.
          $pills = @(Find-UiaElements -Type 'Button' | Where-Object { $_.Current.Name -like "$DraftRecipient, *" })
          Assert-Equal 1 $pills.Count `
            "the resumed composer does not hold the recipient the draft was saved with ($DraftRecipient)"
          Assert-Equal '' (Get-DraftHint) `
            'a composer that has saved nothing yet says nothing about saving (docs/drafts.md)'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'a draft another client stored reopens with its picture'
      Body = {
        # The picture is a `cid:` part, so it is on screen only if the composer was seeded with the
        # message's HTML: a draft seeded with its text alone opens with no picture at all.
        $subject = "Draft suite $DraftRun picture"
        try {
          Add-ForeignDraft -Subject $subject
          Open-StoredDraft -Subject $subject
          Wait-UiaQuiet -CapMs 1500
          # 40 DIPs square, sampled every second pixel each way; half of it is plenty to tell it
          # from none.
          $floor = [int]([Math]::Pow((ConvertTo-UiaPixels 40), 2) / 4 / 2)
          Assert-GreaterThan $floor (Get-EditorRedPixels) `
            'the resumed composer does not draw the picture the stored draft carries: it was not seeded with the draft''s HTML (docs/drafts.md)'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'a reply left as a draft reopens as a reply'
      Body = {
        # The reply's conversation is joined when its composer opens, so even a save made before
        # it was ever sent stores In-Reply-To, and the reopened composer is a reply again. Given a
        # subject of its own so it can be found; the threading headers are what make it a reply.
        $subject = "Draft suite $DraftRun reply"
        try {
          Open-DraftAccountFolder -Folder 'Inbox'
          Invoke-UiaElement (Get-MailRowByTitle 'Harness baseline message') -SettleMs 2000 | Out-Null
          $reply = Wait-UiaElement -AutomationId 'ReadingReply' -TimeoutSec 30
          if (-not $reply) { throw 'the reading pane never offered Reply' }
          Invoke-UiaElement $reply
          if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) { throw 'no composer within 30s of pressing Reply' }
          Set-UiaText (Find-UiaElement -AutomationId 'SubjectBox') $subject
          Invoke-UiaElement (Find-UiaElement -AutomationId 'SaveDraftButton' -Type 'Button')
          if (-not (Wait-DraftSaved)) { throw "the reply was never reported saved; the hint reads '$(Get-DraftHint)'" }
          Close-DraftComposer
          Open-StoredDraft -Subject $subject
          Assert-Equal $DraftCatalog.action_reply (Find-UiaElement -AutomationId 'ComposerTitle').Current.Name `
            'the reply saved as a draft reopened as something else: its save did not carry the message it answers (docs/drafts.md)'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'a double-click on a Drafts row resumes it and opens no reading window'
      Body = {
        $subject = "Draft suite $DraftRun window"
        try {
          New-SavedDraft -Subject $subject
          Close-DraftComposer
          if (-not (Wait-DraftRow -Subject $subject)) { throw "Drafts does not list '$subject'" }
          Invoke-RowClicks -Subject $subject -Times 2
          Assert-True ($null -ne (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) `
            'a double-click on a Drafts row opened no composer, which is where a draft opens'
          Assert-Equal 0 @(Get-ExtraWindows).Count `
            "a double-click on a Drafts row also opened a window ($((Get-ExtraWindows).Title -join ' | ')): the same draft is then open twice, once read-only beside the composer saving over it"
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'Discard over a stored draft asks first, and takes the copy off the server'
      Body = {
        $subject = "Draft suite $DraftRun discard"
        try {
          New-SavedDraft -Subject $subject
          Close-DraftComposer
          # Untouched since it opened, so only the stored copy makes the question due.
          Open-StoredDraft -Subject $subject
          Assert-True (Remove-OpenDraft) `
            'Discard over a draft with a copy in Drafts asked nothing; the copy is something to lose'
          Assert-True (Wait-DraftRow -Subject $subject -Absent) `
            'Discard left the draft in Drafts; it is the one path that removes the stored copy'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'an accepted send takes the stored draft away'
      Body = {
        $subject = "Draft suite $DraftRun send"
        try {
          New-SavedDraft -Subject $subject
          if (-not (Wait-DraftRow -Subject $subject)) { throw "Drafts does not list '$subject' before the send" }
          Invoke-UiaElement (Find-UiaElement -AutomationId 'SendButton' -Type 'Button')
          Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec 10 | Out-Null
          Assert-True (Wait-DraftRow -Subject $subject -Absent) `
            'the message went out and its draft is still in Drafts: the submit did not name its composition, or the composer forgot it before the send could'
        }
        finally { Clear-StoredDraft -Subject $subject }
      }
    },
    @{
      Name = 'a pause after typing in the body saves the draft on its own'
      Body = {
        # Nothing but the body is touched, so the editor's change count is the only thing that can
        # start the idle interval: a header field would start it by itself and prove nothing here.
        Open-NewDraftComposer
        try {
          $editor = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'Editor') -What 'the message editor'
          Invoke-ClicksAt -X ([int]($editor.X + 60)) -Y ([int]($editor.Y + 30))
          Add-Type -AssemblyName System.Windows.Forms
          [System.Windows.Forms.SendKeys]::SendWait('Written in the body and nowhere else')
          # The core's interval is 30s and the host samples at a third of it, so a save is due
          # within 40s of the last keystroke; the rest is the round trip.
          Assert-True (Wait-DraftSaved -TimeoutSec 50) `
            "nothing was saved 50s after typing in the body (the hint reads '$(Get-DraftHint)'): the host is not sampling the editor's change count"
        }
        finally { Remove-OpenDraft | Out-Null }
      }
    }
  )
}
