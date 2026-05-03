param(
    [ValidateSet("Auto", "CurrentUser", "AllUsers")]
    [string]$Scope = "Auto",
    [string]$InstallDir = "",
    [ValidateSet("Ask", "Yes", "No")]
    [string]$Confirm = "Ask"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "fastzip-windows-common.ps1")

function Resolve-UninstallScope {
    param([string]$ResolvedInstallDir)

    foreach ($candidateScope in @("AllUsers", "CurrentUser")) {
        $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $candidateScope
        $uninstallKey = Join-Path $scopeInfo.UninstallRoot "FastZIP"
        if (-not (Test-Path -LiteralPath $uninstallKey)) {
            continue
        }

        $installLocation = (Get-ItemProperty -LiteralPath $uninstallKey -Name InstallLocation -ErrorAction SilentlyContinue).InstallLocation
        if ($installLocation -and ([System.IO.Path]::GetFullPath($installLocation) -eq $ResolvedInstallDir)) {
            return $candidateScope
        }
    }

    if ($ResolvedInstallDir.StartsWith([System.IO.Path]::GetFullPath($env:ProgramFiles), [System.StringComparison]::OrdinalIgnoreCase)) {
        return "AllUsers"
    }

    return "CurrentUser"
}

$resolvedInstallDir = if ($InstallDir) {
    Assert-FastZipInstallDir -Path $InstallDir
} else {
    Assert-FastZipInstallDir -Path $PSScriptRoot
}

$resolvedScope = if ($Scope -eq "Auto") {
    Resolve-UninstallScope -ResolvedInstallDir $resolvedInstallDir
} else {
    $Scope
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
        "-InstallDir",
        $resolvedInstallDir,
        "-Confirm",
        $Confirm
    )

    $elevatedProcess = Start-Process -FilePath (Get-FastZipPowerShellExePath) -ArgumentList $argumentList -Verb RunAs -Wait -PassThru
    exit $elevatedProcess.ExitCode
}

$shouldContinue = Resolve-YesNoChoice -Choice $Confirm -Prompt "Uninstall FastZIP from $resolvedInstallDir?" -Default $true
if (-not $shouldContinue) {
    exit 0
}

Unregister-FastZipWindowsIntegration -Scope $resolvedScope
Set-FastZipAutostart -ExePath (Join-Path $resolvedInstallDir "fastzip.exe") -Enabled $false
Remove-FastZipShortcuts -InstallDir $resolvedInstallDir -Scope $resolvedScope
Remove-FastZipUninstallEntry -Scope $resolvedScope

$cleanupScriptPath = Join-Path $env:TEMP ("fastzip-uninstall-cleanup-" + [Guid]::NewGuid().ToString("N") + ".ps1")
$escapedInstallDir = $resolvedInstallDir.Replace("'", "''")
$escapedCleanupScriptPath = $cleanupScriptPath.Replace("'", "''")
$currentProcessId = $PID

$cleanupScript = @"
`$ErrorActionPreference = "SilentlyContinue"
while (Get-Process -Id $currentProcessId -ErrorAction SilentlyContinue) {
    Start-Sleep -Milliseconds 300
}
if (Test-Path -LiteralPath '$escapedInstallDir') {
    Remove-Item -LiteralPath '$escapedInstallDir' -Recurse -Force -ErrorAction SilentlyContinue
}
if (Test-Path -LiteralPath '$escapedCleanupScriptPath') {
    Remove-Item -LiteralPath '$escapedCleanupScriptPath' -Force -ErrorAction SilentlyContinue
}
"@

Set-Content -LiteralPath $cleanupScriptPath -Value $cleanupScript -Encoding UTF8
Start-Process -FilePath (Get-FastZipPowerShellExePath) -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $cleanupScriptPath) -WindowStyle Hidden | Out-Null

Write-Host ""
Write-Host "FastZIP uninstall started." -ForegroundColor Green
Write-Host "Install directory: $resolvedInstallDir"
Write-Host "Scope: $resolvedScope"
