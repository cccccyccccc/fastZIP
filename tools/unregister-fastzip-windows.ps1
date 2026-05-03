param(
    [ValidateSet("CurrentUser", "AllUsers")]
    [string]$Scope = "CurrentUser"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "fastzip-windows-common.ps1")

Unregister-FastZipWindowsIntegration -Scope $Scope

Write-Host ""
Write-Host "FastZIP Windows shell integration removed." -ForegroundColor Green
Write-Host "Scope: $Scope"
