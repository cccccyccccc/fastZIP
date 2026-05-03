param(
    [string]$ExePath = "",
    [ValidateSet("CurrentUser", "AllUsers")]
    [string]$Scope = "CurrentUser",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$ArchiveAssociations = "Ask",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$ContextMenu = "Ask",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$OpenDefaultApps = "Ask"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "fastzip-windows-common.ps1")

$resolvedExePath = Resolve-FastZipExe -ConfiguredPath $ExePath
if (-not (Test-Path -LiteralPath $resolvedExePath)) {
    throw "FastZIP executable not found: $resolvedExePath"
}

$associateArchives = Resolve-YesNoChoice -Choice $ArchiveAssociations -Prompt "Register FastZIP archive associations and Open With entries?" -Default $true
$registerContextMenus = Resolve-YesNoChoice -Choice $ContextMenu -Prompt "Add the Explorer right-click Compress with FastZIP to menu?" -Default $true
$openDefaultAppsAfterRegistration = $false
if ($associateArchives) {
    $openDefaultAppsAfterRegistration = Resolve-YesNoChoice -Choice $OpenDefaultApps -Prompt "Open Windows Default Apps settings after registration so you can finish setting FastZIP as the default app?" -Default $false
}

Register-FastZipWindowsIntegration `
    -ExePath $resolvedExePath `
    -Scope $Scope `
    -AssociateArchives $associateArchives `
    -RegisterContextMenus $registerContextMenus `
    -OpenDefaultAppsAfterRegistration $openDefaultAppsAfterRegistration

Write-Host ""
Write-Host "FastZIP Windows shell integration updated." -ForegroundColor Green
Write-Host "Executable: $resolvedExePath"
Write-Host "Scope: $Scope"
if ($associateArchives) {
    Write-Host "Archive associations registered."
}
if ($registerContextMenus) {
    Write-Host "Explorer compression context menu registered."
}
