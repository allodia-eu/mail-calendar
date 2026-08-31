# The cargo features a Windows build must ask the shared core for. Dot-source this, then call
# Get-CoreCargoFeatures; it returns the arguments to splat into the cargo invocation, or nothing.
#
# There is one, and it is the Allodia sign-in. The code it turns on is source-available rather than
# GPL and the open tree must build without it, so it is an optional off-by-default dependency that
# an Allodia build asks for -- see BUILDING.md.
#
# It is derived from the registration rather than being a switch of its own, so the two halves
# cannot disagree: a build with the client id gets the code that uses it, and a build without one
# links nothing closed and offers no sign-in. The twin of `core_cargo_features` in
# scripts/dev/lib.sh and of the Gradle build's `credentialValue`.

function Get-CoreCargoFeatures {
  param([Parameter(Mandatory = $true)][string]$Root)

  # Environment first, then the repo's gitignored `.env` -- the order and the file the core's own
  # build script reads. Blank counts as absent, the way a CI run without access to the secrets sets
  # the empty string rather than leaving the name unbound.
  $value = $env:MAILCAL_ALLODIA_CLIENT_ID
  if ([string]::IsNullOrWhiteSpace($value)) {
    $envFile = Join-Path $Root '.env'
    if (Test-Path -LiteralPath $envFile) {
      foreach ($line in Get-Content -LiteralPath $envFile) {
        $trimmed = ($line.Trim()) -replace '^export\s+', ''
        # The last assignment wins, as it does for a shell that sources the file.
        if ($trimmed -match '^MAILCAL_ALLODIA_CLIENT_ID\s*=\s*(.*)$') {
          $value = $Matches[1].Trim().Trim('"', "'")
        }
      }
    }
  }
  if ([string]::IsNullOrWhiteSpace($value)) { return @() }
  return @('--features', 'allodia-license')
}
