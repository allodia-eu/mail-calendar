#!/usr/bin/env pwsh
# Settings → Advanced → Own AI endpoint (docs/ai.md, "Where requests go"; docs/settings.md slot 11):
# the form that gives a build without Allodia's relay somewhere to send AI requests, and so the
# only way such a build reaches Writing style at all.
#
# Why it is here and not in `Mailcal.Tests`: the form's rules (an empty key field keeps the stored
# key, a refusal worded from its code) are pinned there, but whether Advanced draws the form at all
# is WinUI wiring, and a build where it does not is a build with no way into the feature. Every
# headless gate stays green through that.
#
# Read-only on purpose. Saving an endpoint writes the preferences and can write the Credential
# Manager, and a suite must leave the developer's store as it found it; saving and learning are
# driven by hand against scripts/dev/ai-mock.py (docs/debugging.md).

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'Advanced offers an own AI endpoint, whatever the build carries'
      Body = {
        $dialog = Open-SettingsCategory 'Advanced'
        foreach ($id in @('AiEndpointAddress', 'AiEndpointKey', 'AiEndpointModel', 'AiEndpointSave')) {
          $element = Find-UiaElement -AutomationId $id -Root $dialog
          Assert-True ($null -ne $element) (
            "the own endpoint form is always in Advanced (docs/settings.md slot 11), and it is " +
            "missing #$id: without it a build with no relay has no way into writing style")
        }
        $save = Find-UiaElement -AutomationId 'AiEndpointSave' -Root $dialog
        Assert-True $save.Current.IsEnabled (
          'Save is what sets the endpoint up, and it is refused only by the core, never ' +
          'disabled in advance')
      }
    }
  )
}
