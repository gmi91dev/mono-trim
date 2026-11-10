@echo off
rem Install / upgrade mtrim on Windows from cmd.exe
rem Usage: install.cmd [args passed to install.ps1]
setlocal
set "PS1=%~dp0install.ps1"
if not exist "%PS1%" (
    echo [ERROR] install.ps1 not found: %PS1%
    exit /b 1
)
where pwsh >nul 2>nul
if %ERRORLEVEL%==0 (
    pwsh -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*
) else (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*
)
exit /b %ERRORLEVEL%
