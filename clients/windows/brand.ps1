# The brand values the Windows scripts need, read from the Brand.cs the build generated rather than
# parsed out of branding/*.env a second time: scripts/dev/brand.sh owns that resolution order
# (docs/branding.md).
#
#     . (Join-Path $PSScriptRoot 'brand.ps1')
#     Get-BrandConstant AppId
#     Get-BrandConstant WindowsDataDir

function Get-BrandConstant([string] $Name) {
  $brand = Join-Path $PSScriptRoot 'Mailcal/Brand.cs'
  if (-not (Test-Path -LiteralPath $brand)) {
    throw "no generated $brand, so this build's $Name is unknown; build the client first (build-and-run.ps1 -NoRun)."
  }
  $value = [regex]::Match((Get-Content -Raw -LiteralPath $brand), "\b$Name\s*=\s*""([^""]+)""").Groups[1].Value
  if (-not $value) { throw "$brand names no $Name" }
  $value
}
