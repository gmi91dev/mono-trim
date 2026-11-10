<#
.SYNOPSIS
  安装脚本端到端自测（Windows）

.DESCRIPTION
  用本地 HTTP 服务器模拟 Release，验证 install.ps1 → uninstall.ps1 的完整流程：
  下载 → sha256 校验 → 安装 → 版本正确

  依赖：python（提供静态 HTTP 服务）、已构建的 target/release/mtrim.exe
#>
param(
    [string]$Binary = "target/release/mtrim.exe",
    [int]$Port = 8124
)

$ErrorActionPreference = "Stop"

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

$Root = Join-Path $env:TEMP ("mtrim-e2e-" + [guid]::NewGuid().ToString("N"))
$Download = Join-Path $Root "releases/latest/download"
New-Item -ItemType Directory -Force -Path $Download | Out-Null

$Asset = "mtrim-win-x86_64.exe"
if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { $Asset = "mtrim-win-aarch64.exe" }

Copy-Item $Binary (Join-Path $Download $Asset)
$Hash = Get-Sha256 (Join-Path $Download $Asset)
[System.IO.File]::WriteAllText(
    (Join-Path $Download "sha256sums.txt"),
    "$Hash  $Asset`n")

$Server = Start-Process -PassThru -WindowStyle Hidden python `
    -ArgumentList "-m", "http.server", "$Port", "--directory", "$Root"

try {
    $Base = "http://127.0.0.1:$Port"
    $Ready = $false
    for ($i = 0; $i -lt 40; $i++) {
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$Base/releases/latest/download/sha256sums.txt" | Out-Null
            $Ready = $true
            break
        }
        catch {
            Start-Sleep -Milliseconds 500
        }
    }
    if (-not $Ready) { throw "本地 HTTP 服务未能启动" }

    $env:MONO_TRIM_MIRROR = $Base
    $BinDir = Join-Path $Root "bin"

    Write-Host "-> 使用 PowerShell 安装"
    & "$PSScriptRoot\install.ps1" -Dir $BinDir -NoPath -Force

    $Actual = (& (Join-Path $BinDir "mtrim.exe") --version).Trim()
    $Version = (Select-String -Path "Cargo.toml" -Pattern '^version' | Select-Object -First 1).Line.Split('"')[1]
    if ($Actual -ne "mtrim $Version") {
        throw "版本不符：期望 mtrim $Version，实际 $Actual"
    }
    Write-Host "[OK] 安装流程通过（$Actual）"

    Write-Host "-> 执行卸载"
    & "$PSScriptRoot\uninstall.ps1" -Dir $BinDir -KeepPath | Out-Null
    if (Test-Path (Join-Path $BinDir "mtrim.exe")) {
        throw "卸载后仍存在 mtrim.exe"
    }
    Write-Host "[OK] 卸载流程通过"
}
finally {
    Stop-Process -Id $Server.Id -Force -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $Root -ErrorAction SilentlyContinue
}
