<#
.SYNOPSIS
  安装 / 升级 mtrim（Windows）

.DESCRIPTION
  从 GitHub Release 下载当前平台的 mtrim，校验 sha256 后安装到用户目录，并加入用户 PATH。

.PARAMETER Version
  指定版本（如 0.2.0 或 v0.2.0），默认 latest。

.PARAMETER Dir
  安装目录，默认 %LOCALAPPDATA%\Programs\mono-trim。

.PARAMETER Force
  即使版本相同也重新下载安装。

.PARAMETER DryRun
  只打印将要执行的动作，不下载、不写文件。

.PARAMETER NoPath
  不修改用户 PATH（自定义安装目录或测试时使用）。

.EXAMPLE
  .\install.ps1

.EXAMPLE
  .\install.ps1 -Version 0.2.0 -Dir D:\tools\mtrim
#>
param(
    [string]$Version = "latest",
    [string]$Dir,
    [switch]$Force,
    [switch]$DryRun,
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"

# PowerShell 7 在 macOS / Linux 上也能运行，但本脚本只针对 Windows
if ($PSVersionTable.PSEdition -eq "Core" -and -not $IsWindows) {
    throw "此脚本仅用于 Windows；macOS / Linux 请使用 scripts/install.sh"
}

# 兼顾旧版 PowerShell（Get-FileHash / Invoke-WebRequest 不一定可用）
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
    if (Get-Command Invoke-WebRequest -ErrorAction SilentlyContinue) {
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $OutFile
    }
    else {
        $client = New-Object System.Net.WebClient
        try { $client.DownloadFile($Url, $OutFile) } finally { $client.Dispose() }
    }
}

$Repo = if ($env:MONO_TRIM_MIRROR) { $env:MONO_TRIM_MIRROR.TrimEnd('/') } else { "https://github.com/gmi91dev/mono-trim" }

$Arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    "ARM64" { "aarch64" }
    default { "x86_64" }
}
$Asset = "mtrim-win-$Arch.exe"

$Release = if ($Version -eq "latest") { "latest/download" } else { "download/v$($Version.TrimStart('v'))" }
$Base = "$Repo/releases/$Release"
$AssetUrl = "$Base/$Asset"
$SumsUrl = "$Base/sha256sums.txt"

if (-not $Dir) {
    $Dir = Join-Path $env:LOCALAPPDATA "Programs\mono-trim"
}
$Target = Join-Path $Dir "mtrim.exe"

Write-Host "-> 下载地址: $AssetUrl"
Write-Host "-> 安装位置: $Target"

if ($DryRun) {
    Write-Host "(DryRun：未下载、未写入)"
    return
}

New-Item -ItemType Directory -Force -Path $Dir | Out-Null
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ("mtrim-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $Temp | Out-Null

try {
    $Downloaded = Join-Path $Temp $Asset
    $Sums = Join-Path $Temp "sha256sums.txt"

    Write-Host "-> 下载中..."
    Download-File -Url $AssetUrl -OutFile $Downloaded
    Download-File -Url $SumsUrl -OutFile $Sums

    # 校验 sha256
    $Line = Select-String -Path $Sums -Pattern ([regex]::Escape($Asset)) | Select-Object -First 1
    if (-not $Line) {
        throw "校验文件中没有 $Asset 的记录"
    }
    $Expected = $Line.Line.Split(" ")[0].Trim().ToLower()
    $Actual = Get-Sha256 -Path $Downloaded
    if ($Actual -ne $Expected) {
        throw "sha256 校验失败（期望 $Expected，实际 $Actual）"
    }
    Write-Host "-> sha256 校验通过"

    # 版本比较（下载的产物支持 --version）
    $newVersion = ((& $Downloaded --version) | Select-Object -First 1) -replace '^mtrim\s+', ''
    $newVersion = $newVersion.Trim()
    $oldVersion = $null
    if (Test-Path $Target) {
        try {
            $oldVersion = ((& $Target --version) | Select-Object -First 1) -replace '^mtrim\s+', ''
            $oldVersion = $oldVersion.Trim()
        } catch { $oldVersion = $null }
    }

    if (-not $Force -and $oldVersion -and $oldVersion -eq $newVersion) {
        Write-Host "[OK] 已是最新（$newVersion），无需安装"
        return
    }

    # 运行中的 exe 不能被覆盖：先把旧文件改名，再放入新文件
    if (Test-Path $Target) {
        Move-Item -Force -Path $Target -Destination "$Target.old"
    }
    Move-Item -Force -Path $Downloaded -Destination $Target
    Remove-Item -Force -Path "$Target.old" -ErrorAction SilentlyContinue
    if ($oldVersion) {
        Write-Host "[OK] 已升级：$oldVersion → $newVersion"
    } else {
        Write-Host "[OK] 已安装 mtrim $newVersion"
    }
}
finally {
    Remove-Item -Recurse -Force -Path $Temp -ErrorAction SilentlyContinue
}

if ($NoPath) {
    Write-Host "[INFO] 已跳过 PATH 修改（-NoPath）"
    Write-Host "[NEXT] 运行：$Target --version"
    return
}

# 加入用户 PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $userPath) { $userPath = "" }
$entries = $userPath.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)
if ($entries -notcontains $Dir) {
    $newPath = (($entries + $Dir) -join ";")
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "[INFO] 已把 $Dir 加入用户 PATH（新开终端生效）"
}

Write-Host "[NEXT] 新开终端后运行：mtrim --version"
