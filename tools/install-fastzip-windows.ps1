param(
    [string]$ExePath = "",
    [ValidateSet("Ask", "CurrentUser", "AllUsers")]
    [string]$Scope = "Ask",
    [string]$InstallDir = "",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$ArchiveAssociations = "Ask",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$ContextMenu = "Ask",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$OpenDefaultApps = "Ask",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$Autostart = "Ask",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$DesktopShortcut = "Ask"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "fastzip-windows-common.ps1")

$repoRoot = Resolve-FastZipRepoRoot
$resolvedExePath = Resolve-FastZipExe -ConfiguredPath $ExePath -FallbackRoot $repoRoot
if (-not (Test-Path -LiteralPath $resolvedExePath)) {
    throw "FastZIP executable not found: $resolvedExePath`nBuild target\release\fastzip.exe first."
}

$resolvedScope = $Scope
if ($resolvedScope -eq "Ask") {
    if (Test-FastZipIsAdministrator) {
        $installForAllUsers = Prompt-YesNo -Message "Install FastZIP for all Windows users?" -Default $true
    } else {
        $installForAllUsers = Prompt-YesNo -Message "Install FastZIP for all Windows users? This will request administrator permission." -Default $false
    }

    $resolvedScope = if ($installForAllUsers) { "AllUsers" } else { "CurrentUser" }
}

if ($resolvedScope -eq "AllUsers" -and -not (Test-FastZipIsAdministrator)) {
    $argumentList = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $PSCommandPath,
        "-Scope",
        "AllUsers",
        "-ArchiveAssociations",
        $ArchiveAssociations,
        "-ContextMenu",
        $ContextMenu,
        "-OpenDefaultApps",
        $OpenDefaultApps,
        "-Autostart",
        $Autostart,
        "-DesktopShortcut",
        $DesktopShortcut
    )

    if ($ExePath) {
        $argumentList += @("-ExePath", $ExePath)
    }
    if ($InstallDir) {
        $argumentList += @("-InstallDir", $InstallDir)
    }

    $elevatedProcess = Start-Process -FilePath (Get-FastZipPowerShellExePath) -ArgumentList $argumentList -Verb RunAs -Wait -PassThru
    exit $elevatedProcess.ExitCode
}

$defaultInstallDir = Get-FastZipDefaultInstallDir -Scope $resolvedScope
$resolvedInstallDir = if ($InstallDir) {
    $InstallDir
} else {
    Prompt-Text -Message "Install directory" -DefaultValue $defaultInstallDir
}
$resolvedInstallDir = Assert-FastZipInstallDir -Path $resolvedInstallDir

$associateArchives = Resolve-YesNoChoice -Choice $ArchiveAssociations -Prompt "Register FastZIP archive associations and Open With entries?" -Default $true
$registerContextMenus = Resolve-YesNoChoice -Choice $ContextMenu -Prompt "Add the Explorer right-click Compress with FastZIP to menu?" -Default $true
$openDefaultAppsAfterInstallation = $false
if ($associateArchives) {
    $openDefaultAppsAfterInstallation = Resolve-YesNoChoice -Choice $OpenDefaultApps -Prompt "Open Windows Default Apps settings after installation so you can finish setting FastZIP as the default app?" -Default $false
}
$enableAutostart = Resolve-YesNoChoice -Choice $Autostart -Prompt "Start FastZIP automatically when you sign in to Windows?" -Default $false
$createDesktopShortcut = Resolve-YesNoChoice -Choice $DesktopShortcut -Prompt "Create a desktop shortcut?" -Default $false

$installedExePath = Copy-FastZipInstallArtifacts -SourceExePath $resolvedExePath -DestinationDir $resolvedInstallDir -RepoRoot $repoRoot

Install-FastZipShortcuts `
    -ExePath $installedExePath `
    -InstallDir $resolvedInstallDir `
    -Scope $resolvedScope `
    -CreateDesktopShortcut $createDesktopShortcut

Register-FastZipWindowsIntegration `
    -ExePath $installedExePath `
    -Scope $resolvedScope `
    -AssociateArchives $associateArchives `
    -RegisterContextMenus $registerContextMenus `
    -OpenDefaultAppsAfterRegistration $openDefaultAppsAfterInstallation

Set-FastZipAutostart -ExePath $installedExePath -Enabled $enableAutostart

$packageVersion = Get-FastZipPackageVersion -RepoRoot $repoRoot
Write-FastZipUninstallEntry `
    -Scope $resolvedScope `
    -InstallDir $resolvedInstallDir `
    -ExePath $installedExePath `
    -DisplayVersion $packageVersion

Write-Host ""
Write-Host "FastZIP installation completed." -ForegroundColor Green
Write-Host "Executable: $installedExePath"
Write-Host "Install directory: $resolvedInstallDir"
Write-Host "Scope: $resolvedScope"
if ($associateArchives) {
    Write-Host "Archive associations: enabled"
} else {
    Write-Host "Archive associations: skipped"
}
if ($registerContextMenus) {
    Write-Host "Explorer context menu: enabled"
} else {
    Write-Host "Explorer context menu: skipped"
}
if ($enableAutostart) {
    Write-Host "Autostart: enabled for the current Windows user"
} else {
    Write-Host "Autostart: disabled"
}
