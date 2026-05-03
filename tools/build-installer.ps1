param(
    [string]$CompilerPath = "",
    [string]$ScriptPath = "",
    [string]$ReleaseDir = "",
    [string]$OutputDir = "",
    [string]$AppVersion = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-InnoSetupCompiler {
    param([string]$ConfiguredPath)

    if ($ConfiguredPath) {
        $resolvedConfiguredPath = [System.IO.Path]::GetFullPath($ConfiguredPath)
        if (-not (Test-Path -LiteralPath $resolvedConfiguredPath)) {
            throw "Inno Setup compiler not found: $resolvedConfiguredPath"
        }
        return $resolvedConfiguredPath
    }

    $pathMatch = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($pathMatch) {
        return $pathMatch.Source
    }

    foreach ($candidate in @(
        (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
        (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe")
    )) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }

    throw "ISCC.exe was not found. Install Inno Setup 6 first, or pass -CompilerPath."
}

. (Join-Path $PSScriptRoot "fastzip-windows-common.ps1")
$repoRoot = Resolve-FastZipRepoRoot

$resolvedScriptPath = if ($ScriptPath) {
    [System.IO.Path]::GetFullPath($ScriptPath)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoRoot "installer\FastZIP.iss"))
}

if (-not (Test-Path -LiteralPath $resolvedScriptPath)) {
    throw "Installer script not found: $resolvedScriptPath"
}

$resolvedReleaseDir = if ($ReleaseDir) {
    [System.IO.Path]::GetFullPath($ReleaseDir)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target\release"))
}

$mainExePath = Join-Path $resolvedReleaseDir "fastzip.exe"
if (-not (Test-Path -LiteralPath $mainExePath)) {
    throw "Release executable not found: $mainExePath`nBuild target\\release\\fastzip.exe first."
}

$resolvedOutputDir = if ($OutputDir) {
    [System.IO.Path]::GetFullPath($OutputDir)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoRoot "dist"))
}
New-Item -Path $resolvedOutputDir -ItemType Directory -Force | Out-Null

$resolvedAppVersion = if ($AppVersion) {
    $AppVersion
} else {
    $detectedVersion = Get-FastZipPackageVersion -RepoRoot $repoRoot
    if ([string]::IsNullOrWhiteSpace($detectedVersion)) {
        throw "Failed to determine the FastZIP package version from Cargo.toml."
    }
    $detectedVersion
}

$resolvedCompilerPath = Resolve-InnoSetupCompiler -ConfiguredPath $CompilerPath

$arguments = @(
    ('/DAppVersion=' + $resolvedAppVersion),
    ('/DRepoRoot=' + $repoRoot),
    ('/DReleaseDir=' + $resolvedReleaseDir),
    ('/DOutputDir=' + $resolvedOutputDir),
    $resolvedScriptPath
)

Write-Host "Using Inno Setup compiler: $resolvedCompilerPath"
Write-Host "Installer script: $resolvedScriptPath"
Write-Host "Release directory: $resolvedReleaseDir"
Write-Host "Output directory: $resolvedOutputDir"
Write-Host "App version: $resolvedAppVersion"
Write-Host ""

& $resolvedCompilerPath @arguments
if ($LASTEXITCODE -ne 0) {
    throw "ISCC exited with code $LASTEXITCODE"
}
