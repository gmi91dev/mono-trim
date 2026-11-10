<#
.SYNOPSIS
  Install or upgrade mtrim on Windows.

.DESCRIPTION
  Downloads the current platform mtrim from GitHub Releases, verifies its
  sha256 checksum, installs it into a per-user directory and adds it to PATH.

.PARAMETER Version
  Release version to install (for example 0.0.2 or v0.0.2). Defaults to latest.

.PARAMETER Dir
  Install directory. Defaults to %LOCALAPPDATA%\Programs\mono-trim.

.PARAMETER Force
  Reinstall even when the same version is already installed.

.PARAMETER DryRun
  Print what would happen without downloading or writing anything.

.PARAMETER NoPath
  Do not modify the user PATH.

.EXAMPLE
  irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1 | iex

.EXAMPLE
  .\install.ps1 -Version 0.0.2 -Dir D:\tools\mono-trim
#>
param(
    [string]$Version = "latest",
    [string]$Dir,
    [switch]$Force,
    [switch]$DryRun,
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"

# Windows PowerShell 5.1 has no $IsWindows; PowerShell 7 on unix sets it to $false
if (($PSVersionTable.PSEdition -eq "Core") -and (-not $IsWindows)) {
    throw "This script is for Windows only. On macOS / Linux use scripts/install.sh"
}

function Get-Sha256([string]$Path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $bytes = $sha.ComputeHash($stream)
        return ([System.BitConverter]::ToString($bytes) -replace '-', '').ToLower()
    }
    finally {
        $stream.Dispose()
        $sha.Dispose()
    }
}

function Download-File([string]$Url, [string]$OutFile) {
    # Prefer curl.exe: Invoke-WebRequest may fail on some TLS / proxy setups
    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
        & curl.exe -fsSL --retry 2 -o $OutFile $Url
        if ($LASTEXITCODE -eq 0) { return }
    }
    if (Get-Command Invoke-WebRequest -ErrorAction SilentlyContinue) {
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $OutFile
        return
    }
    $client = New-Object System.Net.WebClient
    try { $client.DownloadFile($Url, $OutFile) } finally { $client.Dispose() }
}

$Repo = if ($env:MONO_TRIM_MIRROR) { $env:MONO_TRIM_MIRROR.TrimEnd('/') } else { "https://github.com/gmi91dev/mono-trim" }

$Arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    "ARM64" { "aarch64" }
    default { "x86_64" }
}
$Asset = "mtrim-win-$Arch.exe"

if ($Version -eq "latest") {
    $Base = "$Repo/releases/latest/download"
}
else {
    $Base = "$Repo/releases/download/v$($Version.TrimStart('v'))"
}
$AssetUrl = "$Base/$Asset"
$SumsUrl = "$Base/sha256sums.txt"

if (-not $Dir) {
    $Dir = Join-Path $env:LOCALAPPDATA "Programs\mono-trim"
}
$Target = Join-Path $Dir "mtrim.exe"

Write-Host "-> source: $AssetUrl"
Write-Host "-> target: $Target"

if ($DryRun) {
    Write-Host "(dry run: nothing downloaded or written)"
    return
}

New-Item -ItemType Directory -Force -Path $Dir | Out-Null
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ("mtrim-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $Temp | Out-Null

try {
    $Downloaded = Join-Path $Temp $Asset
    $Sums = Join-Path $Temp "sha256sums.txt"

    Write-Host "-> downloading..."
    Download-File -Url $AssetUrl -OutFile $Downloaded
    Download-File -Url $SumsUrl -OutFile $Sums

    $Line = Select-String -Path $Sums -Pattern ([regex]::Escape($Asset)) | Select-Object -First 1
    if (-not $Line) {
        throw "checksum file has no entry for $Asset"
    }
    $Expected = $Line.Line.Split(" ")[0].Trim().ToLower()
    $Actual = Get-Sha256 -Path $Downloaded
    if ($Actual -ne $Expected) {
        throw "sha256 mismatch (expected $Expected, got $Actual)"
    }
    Write-Host "-> sha256 verified"

    $NewVersion = ((& $Downloaded --version) | Select-Object -First 1) -replace '^mtrim\s+', ''
    $NewVersion = $NewVersion.Trim()

    $OldVersion = $null
    if (Test-Path $Target) {
        try {
            $OldVersion = ((& $Target --version) | Select-Object -First 1) -replace '^mtrim\s+', ''
            $OldVersion = $OldVersion.Trim()
        }
        catch { $OldVersion = $null }
    }

    if ((-not $Force) -and $OldVersion -and ($OldVersion -eq $NewVersion)) {
        Write-Host "[OK] already up to date ($NewVersion)"
        return
    }

    # A running exe cannot be overwritten: rename the old file first
    if (Test-Path $Target) {
        Move-Item -Force -Path $Target -Destination "$Target.old"
    }
    Move-Item -Force -Path $Downloaded -Destination $Target
    Remove-Item -Force -Path "$Target.old" -ErrorAction SilentlyContinue

    if ($OldVersion) {
        Write-Host "[OK] upgraded: $OldVersion -> $NewVersion"
    }
    else {
        Write-Host "[OK] installed mtrim $NewVersion"
    }
}
finally {
    Remove-Item -Recurse -Force -Path $Temp -ErrorAction SilentlyContinue
}

if ($NoPath) {
    Write-Host "[INFO] PATH not modified (-NoPath)"
    Write-Host "[NEXT] run: $Target --version"
    return
}

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $UserPath) { $UserPath = "" }
$Entries = $UserPath.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)
if ($Entries -notcontains $Dir) {
    $NewPath = (($Entries + $Dir) -join ";")
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "[INFO] added $Dir to the user PATH (restart your terminal)"
}

Write-Host "[NEXT] open a new terminal and run: mtrim --version"
