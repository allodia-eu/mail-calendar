#!/usr/bin/env pwsh
# Invoke-WithPinnedSdk { & dotnet … }: runs the block from the checkout, so every `dotnet` in it
# uses the SDK /global.json pins, whatever directory the calling script was started from.
#
# The dotnet host looks for global.json from its working directory upward, never from the project
# it is handed, so a script started outside the checkout builds with whichever SDK is newest on the
# machine. A newer major brings its own compiler and analyzers, and with this tree's warnings as
# errors that is a red build in code nobody changed. Without the pinned SDK the host says so itself
# ("A compatible .NET SDK was not found"), naming the version.
#
# A block rather than a function taking dotnet's arguments: PowerShell drops a bare `--` passed to
# a function, so `run --project X -- -noLogo` would reach dotnet without its separator. Keep a
# pipeline that sets a variable (Tee-Object -Variable) outside the block, which has its own scope.

function Invoke-WithPinnedSdk([scriptblock] $Block) {
  Push-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
  try { & $Block }
  finally { Pop-Location }
}
