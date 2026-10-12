# What the `harness-setup` suites share: reaching the manual form from the first-run screen,
# answering the questions setting an account up asks afterwards, and removing the account again so
# the next suite starts where this one did. Dot-sourced by those suites; it defines functions only.

<#
.SYNOPSIS
From the first-run screen, the manual form with $Type chosen.
.DESCRIPTION
"Set up manually" rather than Continue: detection for test.local reaches no server, and waiting
it out would test the strategies' timeouts rather than the form.
#>
function Open-ManualSetup {
  param(
    [Parameter(Mandatory)] [string] $Address,
    [Parameter(Mandatory)] [ValidateSet('ImapChoice', 'JmapChoice', 'DavChoice')] [string] $Type
  )
  $email = Wait-UiaElement -AutomationId 'DetectEmail' -Type 'Edit' -TimeoutSec 15
  if (-not $email) { throw 'the first-run screen has no address field (#DetectEmail)' }
  Set-UiaText -Element $email -Text $Address
  Invoke-UiaElement (Find-UiaElement -AutomationId 'ManualButton' -Type 'Button') -SettleMs 1200
  $choice = Wait-UiaElement -AutomationId $Type -TimeoutSec 10
  if (-not $choice) { throw "the manual form has no '$Type' account type" }
  $choice.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
  Wait-UiaQuiet -CapMs 1200 -FloorMs 300
}

<#
.SYNOPSIS
Answers what setting an account up asks once it has connected, until the mailbox is on screen.
.DESCRIPTION
In the order they come: the link step, skipped (there is no other account to link); the name a
mailbox sends under, skipped; and the default-mail-app offer, declined. Each is answered only if it
appears, since which of them a route asks is the route's business and asserted elsewhere. Returns
once the Settings gear is up and none of them is.
#>
function Complete-SetupPrompts {
  param([int] $TimeoutSec = 60)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  $quietSince = $null
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $tree = @()
    try { $tree = @(Get-UiaTree) } catch { }
    $byId = @{}
    foreach ($node in $tree) {
      $id = $node.Current.AutomationId
      if ($id -and -not $node.Current.IsOffscreen -and -not $byId.ContainsKey($id)) { $byId[$id] = $node }
    }
    $answer = $null
    if ($byId.ContainsKey('SetupLinksSkip')) { $answer = $byId['SetupLinksSkip'] }
    elseif ($byId.ContainsKey('DefaultMailAppOffer') -or
      @($tree | Where-Object { $_.Current.Name -eq 'Your name' -and $_.Current.ClassName -eq 'Popup' }).Count -gt 0) {
      # Both are ContentDialogs whose CloseButton is the answer that changes nothing.
      $answer = $tree | Where-Object { $_.Current.AutomationId -eq 'CloseButton' -and -not $_.Current.IsOffscreen } |
        Select-Object -First 1
    }
    if ($answer) {
      Invoke-UiaElement $answer -SettleMs 800
      $quietSince = $null
      continue
    }
    if ($byId.ContainsKey('SettingsItem') -and -not $byId.ContainsKey('ConnectButton')) {
      # Two quiet turns in a row: a prompt can follow the one just answered.
      if ($null -eq $quietSince) { $quietSince = [Diagnostics.Stopwatch]::StartNew() }
      elseif ($quietSince.Elapsed.TotalSeconds -ge 3) { return }
    }
    Start-Sleep -Milliseconds 300
  }
  throw "setting the account up did not reach the mailbox within ${TimeoutSec}s"
}

<#
.SYNOPSIS
Removes the one account from its page in Settings, and waits for the first-run screen it leaves.
#>
function Remove-OnlyAccount {
  param([Parameter(Mandatory)] [string] $Address)
  $dialog = Open-SettingsAccount -Address $Address
  Invoke-UiaElement (Find-UiaElement -AutomationId 'AccountRemove' -Root $dialog) -SettleMs 600
  Invoke-UiaElement (Find-UiaElement -AutomationId 'AccountRemoveConfirm' -Root (Get-SettingsDialog)) -SettleMs 1500
  $settings = Find-SettingsDialog
  if ($settings) {
    $close = Find-UiaElement -AutomationId 'CloseButton' -Type 'Button' -Root $settings
    if ($close) { Invoke-UiaElement $close -SettleMs 1200 }
  }
  $email = Wait-UiaElement -AutomationId 'DetectEmail' -Type 'Edit' -TimeoutSec 20
  if (-not $email) { throw 'removing the last account did not return the window to first-run setup' }
}
