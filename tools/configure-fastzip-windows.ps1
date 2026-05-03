param(
    [string]$ExePath = "",
    [ValidateSet("Auto", "CurrentUser", "AllUsers")]
    [string]$Scope = "Auto",
    [ValidateSet("Leave", "Yes", "No")]
    [string]$ArchiveAssociations = "Leave",
    [ValidateSet("Leave", "Yes", "No")]
    [string]$ContextMenu = "Leave",
    [ValidateSet("Leave", "Yes", "No")]
    [string]$OpenDefaultApps = "Leave",
    [ValidateSet("Leave", "Yes", "No")]
    [string]$Autostart = "Leave"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "fastzip-windows-common.ps1")

function Resolve-ConfiguredScope {
    param(
        [string]$RequestedScope,
        [string]$ResolvedExePath
    )

    if ($RequestedScope -ne "Auto") {
        return $RequestedScope
    }

    if ($ResolvedExePath) {
        foreach ($rootPath in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
            if ([string]::IsNullOrWhiteSpace($rootPath)) {
                continue
            }

            $normalizedRoot = [System.IO.Path]::GetFullPath($rootPath).TrimEnd('\') + '\'
            if ($ResolvedExePath.StartsWith($normalizedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                return "AllUsers"
            }
        }
    }

    return "CurrentUser"
}

$resolvedExePath = ""
if (
    $ArchiveAssociations -eq "Yes" -or
    $ContextMenu -eq "Yes" -or
    $Autostart -eq "Yes"
) {
    $resolvedExePath = Resolve-FastZipExe -ConfiguredPath $ExePath
    if (-not (Test-Path -LiteralPath $resolvedExePath)) {
        throw "FastZIP executable not found: $resolvedExePath"
    }
}

$resolvedScope = Resolve-ConfiguredScope -RequestedScope $Scope -ResolvedExePath $resolvedExePath

switch ($ArchiveAssociations) {
    "Yes" { Register-FastZipArchiveAssociations -ExePath $resolvedExePath -Scope $resolvedScope }
    "No" { Unregister-FastZipArchiveAssociations -Scope $resolvedScope }
}

switch ($ContextMenu) {
    "Yes" { Register-FastZipContextMenu -ExePath $resolvedExePath -Scope $resolvedScope }
    "No" { Unregister-FastZipContextMenu -Scope $resolvedScope }
}

switch ($Autostart) {
    "Yes" {
        $autostartExePath = if ($resolvedExePath) {
            $resolvedExePath
        } else {
            Resolve-FastZipExe -ConfiguredPath $ExePath
        }
        Set-FastZipAutostart -ExePath $autostartExePath -Enabled $true
    }
    "No" {
        $autostartExePath = if ($resolvedExePath) {
            $resolvedExePath
        } elseif ($ExePath) {
            [System.IO.Path]::GetFullPath($ExePath)
        } else {
            Join-Path $PSScriptRoot "fastzip.exe"
        }
        Set-FastZipAutostart -ExePath $autostartExePath -Enabled $false
    }
}

if ($OpenDefaultApps -eq "Yes") {
    Open-FastZipDefaultApps
}
