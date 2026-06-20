param(
  [Parameter(Mandatory = $true)]
  [string]$Password,
  [string]$Version = "0.2.2",
  [string]$Installer = "$env:TEMP\mcLauncher-v022\EZMapa_0.2.2_x64-setup.exe",
  [string]$KeyPath = "$env:USERPROFILE\.beacon-signing\beacon.key"
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

if (-not (Test-Path $Installer)) {
  gh release download "v$Version" --repo innvandrer/mcLauncher --pattern "EZMapa_${Version}_x64-setup.exe" --dir (Split-Path $Installer)
}

$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $KeyPath -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $Password

npx tauri signer sign $Installer

$sigPath = "$Installer.sig"
if (-not (Test-Path $sigPath)) {
  throw "Expected signature file not created: $sigPath"
}

$signature = (Get-Content $sigPath -Raw).Trim()
$notes = @"
EZMapa rebrand — the launcher is now fully branded as EZMapa.

## What's new
- **EZMapa branding**: UI, installer, title bar, and app identity (`com.ezmapa.launcher`) are all renamed from Beacon.
- **Automatic data migration**: existing instances, accounts, and settings move from `%APPDATA%\com.beacon.launcher\` on first launch.
- **Installed mods search**: filter mods already on an instance from the instance detail page.
- **Responsive layout**: pages resize cleanly without overlapping panels or a double title bar.
- **Legacy compatibility**: `BEACON_CF_API_KEY`, `BEACON_CLIENT_ID`, and `beacon_index.json` still work; new writes use `ezmapa_index.json`.

## Fixes
- Fixed layout breaking when resizing the launcher window.
- Fixed duplicate title bar in the frameless window build.
"@

$latest = [ordered]@{
  version = "v$Version"
  notes = $notes
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = $signature
      url = "https://github.com/innvandrer/mcLauncher/releases/download/v$Version/EZMapa_${Version}_x64-setup.exe"
    }
  }
}

$latestPath = Join-Path $root "latest.json"
$latest | ConvertTo-Json -Depth 5 | Set-Content $latestPath -Encoding utf8
Write-Host "Wrote $latestPath"

gh release upload "v$Version" $latestPath --repo innvandrer/mcLauncher --clobber
Write-Host "Uploaded latest.json to v$Version release"
