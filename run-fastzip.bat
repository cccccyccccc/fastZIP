@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "CARGO_EXE=%USERPROFILE%\.cargo\bin\cargo.exe"
set "TARGET_EXE=%SCRIPT_DIR%target\release\fastzip.exe"
set "LAUNCH_DIR=%SCRIPT_DIR%target\run"

pushd "%SCRIPT_DIR%" >nul

set "LAUNCH_EXE="
for /f "usebackq delims=" %%I in (`powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$runDir = [System.IO.Path]::GetFullPath('target\\run'); New-Item -ItemType Directory -Force -Path $runDir | Out-Null; $active = @{}; foreach ($process in @(Get-Process fastzip -ErrorAction SilentlyContinue)) { if ($process.Path) { $active[[System.IO.Path]::GetFullPath($process.Path)] = $true } }; Get-ChildItem -Path $runDir -Filter 'fastzip-launch-*.exe' -File -ErrorAction SilentlyContinue | ForEach-Object { $candidate = [System.IO.Path]::GetFullPath($_.FullName); if (-not $active.ContainsKey($candidate)) { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue } }; $launch = Join-Path $runDir ('fastzip-launch-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff') + '-' + $PID + '.exe'); Write-Output $launch"`) do (
    set "LAUNCH_EXE=%%I"
)

if not defined LAUNCH_EXE (
    echo [FastZIP] Failed to prepare launch directory:
    echo   "%LAUNCH_DIR%"
    goto :fail
)

if not exist "%CARGO_EXE%" (
    echo [FastZIP] Rust toolchain not found:
    echo   "%CARGO_EXE%"
    echo.
    echo Fix the local cargo path, then try again.
    goto :fail
)

"%CARGO_EXE%" build --release
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
    echo.
    echo [FastZIP] Build failed with exit code %EXIT_CODE%.
    goto :fail_with_code
)

if not exist "%TARGET_EXE%" (
    echo [FastZIP] Release executable was not produced:
    echo   "%TARGET_EXE%"
    goto :fail
)

copy /y "%TARGET_EXE%" "%LAUNCH_EXE%" >nul
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
    echo [FastZIP] Failed to stage launch executable:
    echo   "%LAUNCH_EXE%"
    goto :fail_with_code
)

start "" /D "%SCRIPT_DIR%" "%LAUNCH_EXE%" %*
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
    echo.
    echo [FastZIP] Launch failed with exit code %EXIT_CODE%.
    goto :fail_with_code
)

popd >nul
exit /b 0

:fail
popd >nul
echo.
pause
exit /b 1

:fail_with_code
popd >nul
echo.
pause
exit /b %EXIT_CODE%
