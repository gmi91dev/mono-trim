<#
.SYNOPSIS
  Uninstall mtrim on Windows.

.DESCRIPTION
  Removes the mtrim binary and drops the install directory from the user PATH.
  Project configuration files (.mono-trim/config.toml, mono-trim.toml) are not touched.

.PARAMETER Dir
  Install directory. Defaults to %LOCALAPPDATA%\Programs\mono-trim.

.PARAMETER KeepPath
  Do not modify the user PATH.

.PARAMETER DryRun
  Print what would happen without changing anything.

.EXAMPLE
  irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/uninstall.ps1 | iex

.EXAMPLE
  .\uninstall.ps1 -Dir D:\tools\mono-trim -KeepPath
#>
param(
    [string]$Dir,
    [switch]$KeepPath,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

if (($PSVersionTable.PSEdition -eq "Core") -and (-not $IsWindows)) {
    throw "This script is for Windows only. On macOS / Linux use scripts/uninstall.sh"
}

if (-not $Dir) {
    $Dir = Join-Path $env:LOCALAPPDATA "Programs\mono-trim"
}
$Target = Join-Path $Dir "mtrim.exe"

Write-Host "-> install dir: $Dir"

# 1. Remove the directory from the user PATH
if (-not $KeepPath) {
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath) {
        $Entries = $UserPath.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)
        $Normalized = $Dir.TrimEnd('\')
        $Filtered = @($Entries | Where-Object { $_.TrimEnd('\') -ne $Normalized })
        if ($Filtered.Count -ne $Entries.Count) {
            if ($DryRun) {
                Write-Host "[DRY] remove $Dir from the user PATH"
            }
            else {
                [Environment]::SetEnvironmentVariable("Path", ($Filtered -join ";"), "User")
                Write-Host "[OK] removed $Dir from the user PATH"
            }
        }
    }
}

# 2. Remove the binary and any leftovers (mtrim.exe.old-*)
$Removed = 0
if (Test-Path $Dir) {
    $Files = @(Get-ChildItem -Path $Dir -Filter "mtrim.exe*" -File -ErrorAction SilentlyContinue)
    foreach ($File in $Files) {
        if ($DryRun) {
            Write-Host "[DRY] remove $($File.FullName)"
            $Removed++
            continue
        }
        try {
            Remove-Item -Force -Path $File.FullName
            Write-Host "[OK] removed $($File.Name)"
            $Removed++
        }
        catch {
            # The running image cannot be deleted: rename it so it leaves PATH immediately
            $Old = "$($File.FullName).old-$PID"
            try {
                Move-Item -Force -Path $File.FullName -Destination $Old
                Write-Host "[INFO] $($File.Name) is in use; renamed to $Old (delete it after closing the terminal)"
                $Removed++
            }
            catch {
                Write-Warning "cannot remove $($File.FullName): $($_.Exception.Message)"
            }
        }
    }

    if (-not $DryRun) {
        $Left = @(Get-ChildItem -Force -Path $Dir -ErrorAction SilentlyContinue)
        if ($Left.Count -eq 0) {
            Remove-Item -Force -Path $Dir
            Write-Host "[OK] removed directory $Dir"
        }
    }
}

if ($Removed -eq 0 -and -not (Test-Path $Dir)) {
    Write-Host "[INFO] mtrim is not installed in $Dir"
}
elseif (-not $DryRun) {
    Write-Host "[OK] mtrim uninstalled"
}

Write-Host "[INFO] project config files (.mono-trim/config.toml, mono-trim.toml) are not touched"
