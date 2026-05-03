param(
    [ValidateSet('go1.13', 'linux-6.8')]
    [string[]]$Datasets = @('go1.13', 'linux-6.8'),
    [ValidateSet('fastest', 'fast', 'normal', 'maximum', 'ultra')]
    [string[]]$Levels = @('fastest', 'fast'),
    [int]$Threads = [Environment]::ProcessorCount,
    [switch]$KeepOutputs
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$fastzipExe = Join-Path $repoRoot 'target\release\fastzip-cli.exe'
$bandizipExe = 'C:\Program Files\Bandizip\bz.exe'
$sevenZipExe = 'C:\Program Files\7-Zip\7z.exe'
$outputRoot = Join-Path $repoRoot 'tmp\zip_perf\outputs'
$resultRoot = Join-Path $repoRoot 'tmp\zip_perf\results'

New-Item -ItemType Directory -Force -Path $outputRoot, $resultRoot | Out-Null

if (-not $KeepOutputs) {
    Get-ChildItem -LiteralPath $outputRoot -File -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

if (-not (Test-Path -LiteralPath $fastzipExe)) {
    throw "FastZIP CLI not found: $fastzipExe"
}
if (-not (Test-Path -LiteralPath $bandizipExe)) {
    throw "Bandizip CLI not found: $bandizipExe"
}
if (-not (Test-Path -LiteralPath $sevenZipExe)) {
    throw "7-Zip CLI not found: $sevenZipExe"
}

$datasetTable = @{
    'go1.13' = @{
        Name = 'go1.13'
        SourcePath = Join-Path $repoRoot 'tmp\zip_perf\datasets\go1.13'
    }
    'linux-6.8' = @{
        Name = 'linux-6.8'
        SourcePath = Join-Path $repoRoot 'tmp\zip_perf\datasets\linux-6.8\linux-6.8'
    }
}

function Get-InputBytes {
    param([string]$Path)

    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -File | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) {
        return 0
    }
    return [uint64]$sum
}

function Get-BandizipLevel {
    param([string]$Level)

    switch ($Level) {
        'fastest' { return 1 }
        'fast' { return 3 }
        'normal' { return 5 }
        'maximum' { return 7 }
        'ultra' { return 9 }
        default { throw "Unsupported Bandizip level: $Level" }
    }
}

function Get-7ZipLevel {
    param([string]$Level)

    switch ($Level) {
        'fastest' { return 1 }
        'fast' { return 3 }
        'normal' { return 5 }
        'maximum' { return 7 }
        'ultra' { return 9 }
        default { throw "Unsupported 7-Zip level: $Level" }
    }
}

function Invoke-ArchiveRun {
    param(
        [string]$DatasetName,
        [string]$Level,
        [string]$Tool,
        [string]$InputPath,
        [string]$ArchivePath,
        [uint64]$InputBytes
    )

    if (Test-Path -LiteralPath $ArchivePath) {
        Remove-Item -LiteralPath $ArchivePath -Force
    }

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    switch ($Tool) {
        'fastzip' {
            & $fastzipExe compress $InputPath -o $ArchivePath --threads $Threads --level $Level --method deflate | Out-Null
        }
        'bandizip' {
            $bandizipLevel = Get-BandizipLevel $Level
            & $bandizipExe c -y -r -fmt:zip "-l:$bandizipLevel" $ArchivePath $InputPath | Out-Null
        }
        '7zip' {
            $sevenZipLevel = Get-7ZipLevel $Level
            & $sevenZipExe a -tzip "-mx=$sevenZipLevel" "-mmt=$Threads" -bd -y $ArchivePath $InputPath | Out-Null
        }
        default {
            throw "Unsupported tool: $Tool"
        }
    }
    $stopwatch.Stop()

    if ($LASTEXITCODE -ne 0) {
        throw "$Tool failed with exit code $LASTEXITCODE"
    }

    $outputBytes = (Get-Item -LiteralPath $ArchivePath).Length
    [pscustomobject]@{
        dataset = $DatasetName
        tool = $Tool
        level = $Level
        seconds = [math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
        throughput_mib_s = [math]::Round((($InputBytes / 1MB) / $stopwatch.Elapsed.TotalSeconds), 2)
        ratio = [math]::Round(($outputBytes / $InputBytes), 4)
        output_mib = [math]::Round(($outputBytes / 1MB), 2)
    }
}

$results = New-Object System.Collections.Generic.List[object]

foreach ($datasetKey in $Datasets) {
    $dataset = $datasetTable[$datasetKey]
    if ($null -eq $dataset) {
        throw "Unknown dataset: $datasetKey"
    }
    if (-not (Test-Path -LiteralPath $dataset.SourcePath)) {
        throw "Dataset not found: $($dataset.SourcePath)"
    }

    $inputBytes = Get-InputBytes $dataset.SourcePath
    foreach ($level in $Levels) {
        foreach ($tool in @('fastzip', 'bandizip', '7zip')) {
            $archiveName = '{0}-{1}-{2}.zip' -f $dataset.Name, $tool, $level
            $archivePath = Join-Path $outputRoot $archiveName
            $results.Add((Invoke-ArchiveRun $dataset.Name $level $tool $dataset.SourcePath $archivePath $inputBytes))
        }
    }
}

$timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$datasetTag = ($Datasets -join '_').Replace('.', '')
$resultPath = Join-Path $resultRoot ('zip_compare_{0}_{1}.json' -f $datasetTag, $timestamp)
$results | ConvertTo-Json | Set-Content -Encoding utf8 $resultPath
$results | ConvertTo-Json

if (-not $KeepOutputs) {
    Get-ChildItem -LiteralPath $outputRoot -File -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
}
