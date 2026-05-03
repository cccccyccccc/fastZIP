$FastZipProgId = "FastZIP.Archive"

function Resolve-FastZipRepoRoot {
    return [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
}

function Resolve-FastZipExe {
    param(
        [string]$ConfiguredPath,
        [string]$FallbackRoot = ""
    )

    if ($ConfiguredPath) {
        return [System.IO.Path]::GetFullPath($ConfiguredPath)
    }

    $localExePath = Join-Path $PSScriptRoot "fastzip.exe"
    if (Test-Path -LiteralPath $localExePath) {
        return [System.IO.Path]::GetFullPath($localExePath)
    }

    $repoRoot = if ($FallbackRoot) {
        [System.IO.Path]::GetFullPath($FallbackRoot)
    } else {
        Resolve-FastZipRepoRoot
    }

    return [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target\release\fastzip.exe"))
}

function Resolve-FastZipIconPath {
    param(
        [string]$ExePath = "",
        [string]$FallbackRoot = ""
    )

    if ($ExePath) {
        $resolvedExePath = [System.IO.Path]::GetFullPath($ExePath)
        $sidecarIconPath = [System.IO.Path]::ChangeExtension($resolvedExePath, ".ico")
        if (Test-Path -LiteralPath $sidecarIconPath) {
            return [System.IO.Path]::GetFullPath($sidecarIconPath)
        }
    }

    $localIconPath = Join-Path $PSScriptRoot "fastzip.ico"
    if (Test-Path -LiteralPath $localIconPath) {
        return [System.IO.Path]::GetFullPath($localIconPath)
    }

    $repoRoot = if ($FallbackRoot) {
        [System.IO.Path]::GetFullPath($FallbackRoot)
    } else {
        Resolve-FastZipRepoRoot
    }

    $repoIconPath = Join-Path $repoRoot "assets\fastzip.ico"
    if (Test-Path -LiteralPath $repoIconPath) {
        return [System.IO.Path]::GetFullPath($repoIconPath)
    }

    if ($ExePath) {
        return [System.IO.Path]::GetFullPath($ExePath)
    }

    throw "FastZIP icon file not found."
}

function Format-FastZipIconLocation {
    param([string]$Path)

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    if ([System.IO.Path]::GetExtension($resolvedPath).ToLowerInvariant() -eq ".ico") {
        return $resolvedPath
    }

    return "$resolvedPath,0"
}

function Get-FastZipPackageVersion {
    param([string]$RepoRoot = "")

    $resolvedRepoRoot = if ($RepoRoot) {
        [System.IO.Path]::GetFullPath($RepoRoot)
    } else {
        Resolve-FastZipRepoRoot
    }

    $cargoTomlPath = Join-Path $resolvedRepoRoot "Cargo.toml"
    if (-not (Test-Path -LiteralPath $cargoTomlPath)) {
        return ""
    }

    $match = Select-String -Path $cargoTomlPath -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($null -eq $match) {
        return ""
    }

    return $match.Matches[0].Groups[1].Value
}

function Get-FastZipPowerShellExePath {
    return [System.IO.Path]::GetFullPath((Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"))
}

function Test-FastZipIsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Prompt-YesNo {
    param(
        [string]$Message,
        [bool]$Default = $true
    )

    $suffix = if ($Default) { "[Y/n]" } else { "[y/N]" }
    while ($true) {
        $answer = Read-Host "$Message $suffix"
        if ([string]::IsNullOrWhiteSpace($answer)) {
            return $Default
        }

        switch ($answer.Trim().ToLowerInvariant()) {
            "y" { return $true }
            "yes" { return $true }
            "n" { return $false }
            "no" { return $false }
        }
    }
}

function Prompt-Text {
    param(
        [string]$Message,
        [string]$DefaultValue = ""
    )

    $suffix = if ([string]::IsNullOrWhiteSpace($DefaultValue)) {
        ""
    } else {
        " [$DefaultValue]"
    }

    $answer = Read-Host "$Message$suffix"
    if ([string]::IsNullOrWhiteSpace($answer)) {
        return $DefaultValue
    }

    return $answer.Trim()
}

function Resolve-YesNoChoice {
    param(
        [ValidateSet("Ask", "Yes", "No")]
        [string]$Choice = "Ask",
        [string]$Prompt,
        [bool]$Default = $true
    )

    switch ($Choice) {
        "Yes" { return $true }
        "No" { return $false }
        default { return (Prompt-YesNo -Message $Prompt -Default $Default) }
    }
}

function Get-FastZipDefaultInstallDir {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope
    )

    switch ($Scope) {
        "AllUsers" {
            return [System.IO.Path]::GetFullPath((Join-Path $env:ProgramFiles "FastZIP"))
        }
        default {
            return [System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA "Programs\FastZIP"))
        }
    }
}

function Assert-FastZipInstallDir {
    param([string]$Path)

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $rootPath = [System.IO.Path]::GetPathRoot($resolvedPath)
    if ($resolvedPath.TrimEnd('\') -eq $rootPath.TrimEnd('\')) {
        throw "Refusing to use a drive root as the FastZIP install directory: $resolvedPath"
    }

    return $resolvedPath
}

function Ensure-Key {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -Path $Path -Force | Out-Null
    }
}

function Ensure-ParentDirectory {
    param([string]$Path)

    $parentPath = Split-Path -Path $Path -Parent
    if ($parentPath) {
        New-Item -Path $parentPath -ItemType Directory -Force | Out-Null
    }
}

function Set-DefaultValue {
    param(
        [string]$Path,
        [string]$Value
    )

    Ensure-Key $Path
    Set-Item -Path $Path -Value $Value -Force
}

function Set-StringValue {
    param(
        [string]$Path,
        [string]$Name,
        [string]$Value
    )

    Ensure-Key $Path
    New-ItemProperty -Path $Path -Name $Name -Value $Value -PropertyType String -Force | Out-Null
}

function Set-DwordValue {
    param(
        [string]$Path,
        [string]$Name,
        [int]$Value
    )

    Ensure-Key $Path
    New-ItemProperty -Path $Path -Name $Name -Value $Value -PropertyType DWord -Force | Out-Null
}

function Remove-RegistryTree {
    param([string]$Path)

    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
}

function Remove-RegistryValue {
    param(
        [string]$Path,
        [string]$Name
    )

    if (Test-Path -LiteralPath $Path) {
        Remove-ItemProperty -LiteralPath $Path -Name $Name -Force -ErrorAction SilentlyContinue
    }
}

function Get-FastZipRegistryScopeInfo {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope
    )

    switch ($Scope) {
        "AllUsers" {
            return @{
                ClassesRoot = "Registry::HKEY_LOCAL_MACHINE\Software\Classes"
                AppRoot = "Registry::HKEY_LOCAL_MACHINE\Software\FastZIP"
                RegisteredApplicationsKey = "Registry::HKEY_LOCAL_MACHINE\Software\RegisteredApplications"
                UninstallRoot = "Registry::HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\Uninstall"
                ProgramsFolder = [Environment]::GetFolderPath("CommonPrograms")
                DesktopFolder = [Environment]::GetFolderPath("CommonDesktopDirectory")
            }
        }
        default {
            return @{
                ClassesRoot = "Registry::HKEY_CURRENT_USER\Software\Classes"
                AppRoot = "Registry::HKEY_CURRENT_USER\Software\FastZIP"
                RegisteredApplicationsKey = "Registry::HKEY_CURRENT_USER\Software\RegisteredApplications"
                UninstallRoot = "Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall"
                ProgramsFolder = [Environment]::GetFolderPath("Programs")
                DesktopFolder = [Environment]::GetFolderPath("DesktopDirectory")
            }
        }
    }
}

function Get-FastZipArchiveExtensions {
    return @(
        ".7z", ".zip", ".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2",
        ".tar.xz", ".txz", ".gz", ".bz2", ".bzip2", ".xz",
        ".zst", ".zstd", ".tar.zst", ".tzst", ".lz4", ".tar.lz4", ".tlz4"
    )
}

function Get-FastZipContextFormats {
    return @(
        @{ Name = "zip"; Label = "ZIP"; FormatArg = "zip" },
        @{ Name = "sevenzip"; Label = "7Z"; FormatArg = "7z" },
        @{ Name = "tar"; Label = "TAR"; FormatArg = "tar" },
        @{ Name = "targz"; Label = "TAR.GZ"; FormatArg = "tar.gz" },
        @{ Name = "tarbz2"; Label = "TAR.BZ2"; FormatArg = "tar.bz2" },
        @{ Name = "tarxz"; Label = "TAR.XZ"; FormatArg = "tar.xz" },
        @{ Name = "gz"; Label = "GZ"; FormatArg = "gz" },
        @{ Name = "bz2"; Label = "BZ2"; FormatArg = "bz2" },
        @{ Name = "xz"; Label = "XZ"; FormatArg = "xz" },
        @{ Name = "zst"; Label = "ZST"; FormatArg = "zst" },
        @{ Name = "tarzst"; Label = "TAR.ZST"; FormatArg = "tar.zst" },
        @{ Name = "lz4"; Label = "LZ4"; FormatArg = "lz4" },
        @{ Name = "tarlz4"; Label = "TAR.LZ4"; FormatArg = "tar.lz4" }
    )
}

function Open-FastZipDefaultApps {
    try {
        Start-Process "ms-settings:defaultapps?registeredAppUser=FastZIP" | Out-Null
    } catch {
        Start-Process "ms-settings:defaultapps" | Out-Null
    }
}

function Register-FastZipArchiveAssociations {
    param(
        [string]$ExePath,
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser"
    )

    $resolvedExePath = [System.IO.Path]::GetFullPath($ExePath)
    if (-not (Test-Path -LiteralPath $resolvedExePath)) {
        throw "FastZIP executable not found: $resolvedExePath"
    }
    $resolvedIconPath = Resolve-FastZipIconPath -ExePath $resolvedExePath
    $iconLocation = Format-FastZipIconLocation -Path $resolvedIconPath

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    $archiveExtensions = Get-FastZipArchiveExtensions
    $quotedExe = '"' + $resolvedExePath + '"'
    $openArchiveCommand = '{0} gui --archive "%1"' -f $quotedExe

    $progIdKey = Join-Path $scopeInfo.ClassesRoot $FastZipProgId
    Set-DefaultValue $progIdKey "FastZIP Archive"
    Set-StringValue $progIdKey "FriendlyTypeName" "FastZIP Archive"
    Set-StringValue $progIdKey "AppUserModelID" "FastZIP"
    Set-DefaultValue (Join-Path $progIdKey "DefaultIcon") $iconLocation
    Set-DefaultValue (Join-Path $progIdKey "shell\open\command") $openArchiveCommand

    $appKey = Join-Path $scopeInfo.ClassesRoot "Applications\fastzip.exe"
    Set-StringValue $appKey "FriendlyAppName" "FastZIP"
    Set-DefaultValue (Join-Path $appKey "shell\open\command") $openArchiveCommand

    $supportedTypesKey = Join-Path $appKey "SupportedTypes"
    foreach ($extension in $archiveExtensions) {
        Set-StringValue $supportedTypesKey $extension ""
    }

    foreach ($extension in $archiveExtensions) {
        $extensionKey = Join-Path $scopeInfo.ClassesRoot $extension
        Set-StringValue (Join-Path $extensionKey "OpenWithProgids") $FastZipProgId ""
        Ensure-Key (Join-Path $extensionKey "OpenWithList\fastzip.exe")
    }

    $capabilitiesKey = Join-Path $scopeInfo.AppRoot "Capabilities"
    Set-StringValue $capabilitiesKey "ApplicationName" "FastZIP"
    Set-StringValue $capabilitiesKey "ApplicationDescription" "FastZIP archive manager"

    $fileAssociationsKey = Join-Path $capabilitiesKey "FileAssociations"
    foreach ($extension in $archiveExtensions) {
        Set-StringValue $fileAssociationsKey $extension $FastZipProgId
    }

    Set-StringValue $scopeInfo.RegisteredApplicationsKey "FastZIP" ((Join-Path "Software\FastZIP" "Capabilities").Replace("/", "\"))
}

function Unregister-FastZipArchiveAssociations {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser"
    )

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope

    Remove-RegistryTree (Join-Path $scopeInfo.ClassesRoot $FastZipProgId)
    Remove-RegistryTree $scopeInfo.AppRoot
    Remove-RegistryValue $scopeInfo.RegisteredApplicationsKey "FastZIP"

    foreach ($extension in (Get-FastZipArchiveExtensions)) {
        $extensionKey = Join-Path $scopeInfo.ClassesRoot $extension
        Remove-RegistryValue (Join-Path $extensionKey "OpenWithProgids") $FastZipProgId
        Remove-RegistryTree (Join-Path $extensionKey "OpenWithList\fastzip.exe")
    }

    Remove-RegistryTree (Join-Path $scopeInfo.ClassesRoot "Applications\fastzip.exe")
}

function Register-FastZipContextMenu {
    param(
        [string]$ExePath,
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser"
    )

    $resolvedExePath = [System.IO.Path]::GetFullPath($ExePath)
    if (-not (Test-Path -LiteralPath $resolvedExePath)) {
        throw "FastZIP executable not found: $resolvedExePath"
    }
    $resolvedIconPath = Resolve-FastZipIconPath -ExePath $resolvedExePath

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    $contextFormats = Get-FastZipContextFormats
    $quotedExe = '"' + $resolvedExePath + '"'

    $menuRoot = Join-Path $scopeInfo.ClassesRoot "AllFilesystemObjects\shell\FastZIPCompressTo"
    Set-StringValue $menuRoot "MUIVerb" "Compress with FastZIP to"
    Set-StringValue $menuRoot "Icon" $resolvedIconPath
    Set-StringValue $menuRoot "MultiSelectModel" "Player"

    $cascadeShell = Join-Path (Join-Path $menuRoot "ExtendedSubCommandsKey") "Shell"
    foreach ($item in $contextFormats) {
        $verbKey = Join-Path $cascadeShell $item.Name
        Set-StringValue $verbKey "MUIVerb" $item.Label
        Set-StringValue $verbKey "Icon" $resolvedIconPath
        Set-StringValue $verbKey "MultiSelectModel" "Player"
        Set-DefaultValue (Join-Path $verbKey "command") ('{0} shell-compress --format {1} "%1"' -f $quotedExe, $item.FormatArg)
    }
}

function Unregister-FastZipContextMenu {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser"
    )

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    Remove-RegistryTree (Join-Path $scopeInfo.ClassesRoot "AllFilesystemObjects\shell\FastZIPCompressTo")
}

function Register-FastZipWindowsIntegration {
    param(
        [string]$ExePath,
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser",
        [bool]$AssociateArchives = $true,
        [bool]$RegisterContextMenus = $true,
        [bool]$OpenDefaultAppsAfterRegistration = $false
    )

    if ($AssociateArchives) {
        Register-FastZipArchiveAssociations -ExePath $ExePath -Scope $Scope
    }

    if ($RegisterContextMenus) {
        Register-FastZipContextMenu -ExePath $ExePath -Scope $Scope
    }

    if ($AssociateArchives -and $OpenDefaultAppsAfterRegistration) {
        Open-FastZipDefaultApps
    }
}

function Unregister-FastZipWindowsIntegration {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser"
    )

    Unregister-FastZipContextMenu -Scope $Scope
    Unregister-FastZipArchiveAssociations -Scope $Scope
}

function Set-FastZipAutostart {
    param(
        [string]$ExePath,
        [bool]$Enabled
    )

    $runKey = "Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run"
    if ($Enabled) {
        $resolvedExePath = [System.IO.Path]::GetFullPath($ExePath)
        Set-StringValue $runKey "FastZIP" ('"{0}" gui' -f $resolvedExePath)
    } else {
        Remove-RegistryValue $runKey "FastZIP"
    }
}

function New-FastZipShortcut {
    param(
        [string]$ShortcutPath,
        [string]$TargetPath,
        [string]$Arguments = "",
        [string]$WorkingDirectory = "",
        [string]$IconLocation = ""
    )

    Ensure-ParentDirectory -Path $ShortcutPath

    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($ShortcutPath)
    $shortcut.TargetPath = $TargetPath
    if ($Arguments) {
        $shortcut.Arguments = $Arguments
    }
    if ($WorkingDirectory) {
        $shortcut.WorkingDirectory = $WorkingDirectory
    }
    if ($IconLocation) {
        $shortcut.IconLocation = $IconLocation
    }
    $shortcut.Save()
}

function Install-FastZipShortcuts {
    param(
        [string]$ExePath,
        [string]$InstallDir,
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser",
        [bool]$CreateDesktopShortcut = $false
    )

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    $programsFolder = Join-Path $scopeInfo.ProgramsFolder "FastZIP"
    $powerShellExe = Get-FastZipPowerShellExePath
    $uninstallScriptPath = Join-Path $InstallDir "uninstall-fastzip-windows.ps1"
    $resolvedIconPath = Resolve-FastZipIconPath -ExePath $ExePath
    $iconLocation = Format-FastZipIconLocation -Path $resolvedIconPath

    New-Item -Path $programsFolder -ItemType Directory -Force | Out-Null

    New-FastZipShortcut `
        -ShortcutPath (Join-Path $programsFolder "FastZIP.lnk") `
        -TargetPath $ExePath `
        -Arguments "gui" `
        -WorkingDirectory $InstallDir `
        -IconLocation $iconLocation

    New-FastZipShortcut `
        -ShortcutPath (Join-Path $programsFolder "Uninstall FastZIP.lnk") `
        -TargetPath $powerShellExe `
        -Arguments ('-NoProfile -ExecutionPolicy Bypass -File "' + $uninstallScriptPath + '"') `
        -WorkingDirectory $InstallDir `
        -IconLocation $iconLocation

    if ($CreateDesktopShortcut) {
        New-FastZipShortcut `
            -ShortcutPath (Join-Path $scopeInfo.DesktopFolder "FastZIP.lnk") `
            -TargetPath $ExePath `
            -Arguments "gui" `
            -WorkingDirectory $InstallDir `
            -IconLocation $iconLocation
    }
}

function Remove-FastZipShortcuts {
    param(
        [string]$InstallDir,
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope = "CurrentUser"
    )

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    $programsFolder = Join-Path $scopeInfo.ProgramsFolder "FastZIP"
    $desktopShortcutPath = Join-Path $scopeInfo.DesktopFolder "FastZIP.lnk"

    if (Test-Path -LiteralPath $desktopShortcutPath) {
        Remove-Item -LiteralPath $desktopShortcutPath -Force -ErrorAction SilentlyContinue
    }

    if (Test-Path -LiteralPath $programsFolder) {
        Remove-Item -LiteralPath $programsFolder -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Write-FastZipUninstallEntry {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope,
        [string]$InstallDir,
        [string]$ExePath,
        [string]$DisplayVersion = ""
    )

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    $uninstallKey = Join-Path $scopeInfo.UninstallRoot "FastZIP"
    $uninstallScriptPath = Join-Path $InstallDir "uninstall-fastzip-windows.ps1"
    $uninstallCommand = 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "{0}" -Scope {1}' -f $uninstallScriptPath, $Scope
    $quietUninstallCommand = $uninstallCommand + " -Confirm No"
    $resolvedIconPath = Resolve-FastZipIconPath -ExePath $ExePath

    Set-StringValue $uninstallKey "DisplayName" "FastZIP"
    if ($DisplayVersion) {
        Set-StringValue $uninstallKey "DisplayVersion" $DisplayVersion
    }
    Set-StringValue $uninstallKey "Publisher" "FastZIP"
    Set-StringValue $uninstallKey "InstallLocation" $InstallDir
    Set-StringValue $uninstallKey "DisplayIcon" $resolvedIconPath
    Set-StringValue $uninstallKey "UninstallString" $uninstallCommand
    Set-StringValue $uninstallKey "QuietUninstallString" $quietUninstallCommand
    Set-StringValue $uninstallKey "InstallDate" (Get-Date -Format "yyyyMMdd")
    Set-DwordValue $uninstallKey "NoModify" 1
    Set-DwordValue $uninstallKey "NoRepair" 1

    $installedFiles = Get-ChildItem -LiteralPath $InstallDir -File -Recurse -ErrorAction SilentlyContinue
    if ($null -ne $installedFiles) {
        $estimatedBytes = ($installedFiles | Measure-Object -Property Length -Sum).Sum
        if ($estimatedBytes) {
            Set-DwordValue $uninstallKey "EstimatedSize" ([int][Math]::Ceiling($estimatedBytes / 1KB))
        }
    }
}

function Remove-FastZipUninstallEntry {
    param(
        [ValidateSet("CurrentUser", "AllUsers")]
        [string]$Scope
    )

    $scopeInfo = Get-FastZipRegistryScopeInfo -Scope $Scope
    Remove-RegistryTree (Join-Path $scopeInfo.UninstallRoot "FastZIP")
}

function Copy-FastZipInstallArtifacts {
    param(
        [string]$SourceExePath,
        [string]$DestinationDir,
        [string]$RepoRoot = ""
    )

    $resolvedRepoRoot = if ($RepoRoot) {
        [System.IO.Path]::GetFullPath($RepoRoot)
    } else {
        Resolve-FastZipRepoRoot
    }

    $resolvedDestinationDir = Assert-FastZipInstallDir -Path $DestinationDir
    New-Item -Path $resolvedDestinationDir -ItemType Directory -Force | Out-Null

    $resolvedSourceExePath = [System.IO.Path]::GetFullPath($SourceExePath)
    if (-not (Test-Path -LiteralPath $resolvedSourceExePath)) {
        throw "FastZIP executable not found: $resolvedSourceExePath"
    }

    $installedExePath = Join-Path $resolvedDestinationDir "fastzip.exe"
    Copy-Item -LiteralPath $resolvedSourceExePath -Destination $installedExePath -Force

    foreach ($relativePath in @(
        "assets\fastzip.ico",
        "tools\fastzip-windows-common.ps1",
        "tools\register-fastzip-windows.ps1",
        "tools\unregister-fastzip-windows.ps1",
        "tools\configure-fastzip-windows.ps1",
        "tools\uninstall-fastzip-windows.ps1"
    )) {
        $sourcePath = Join-Path $resolvedRepoRoot $relativePath
        if (-not (Test-Path -LiteralPath $sourcePath)) {
            throw "Installer support file not found: $sourcePath"
        }

        $destinationPath = Join-Path $resolvedDestinationDir ([System.IO.Path]::GetFileName($relativePath))
        Copy-Item -LiteralPath $sourcePath -Destination $destinationPath -Force
    }

    return $installedExePath
}
