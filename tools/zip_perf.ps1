param(
    [int]$Iterations = 0,
    [ValidateSet('small', 'medium', 'large')]
    [string[]]$Groups = @('small', 'medium'),
    [int]$SmallIterations = 4,
    [int]$MediumIterations = 2,
    [int]$LargeIterations = 1,
    [int]$Threads = [Environment]::ProcessorCount,
    [ValidateSet('fastest', 'fast', 'normal', 'maximum', 'ultra')]
    [string]$Level = 'fastest',
    [switch]$SkipBandizip,
    [switch]$Skip7Zip,
    [switch]$KeepOutputs
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$fastzipCliExe = Join-Path $repoRoot 'target\release\fastzip-cli.exe'
$fastzipGuiExe = Join-Path $repoRoot 'target\release\fastzip.exe'
$fastzipExe = if (Test-Path -LiteralPath $fastzipCliExe) { $fastzipCliExe } else { $fastzipGuiExe }
$bandizipExe = 'C:\Program Files\Bandizip\bz.exe'
$sevenZipExe = 'C:\Program Files\7-Zip\7z.exe'
$perfRoot = Join-Path $repoRoot 'tmp\zip_perf'
$downloadRoot = Join-Path $perfRoot 'downloads'
$datasetRoot = Join-Path $perfRoot 'datasets'
$outputRoot = Join-Path $perfRoot 'outputs'
$resultRoot = Join-Path $perfRoot 'results'

New-Item -ItemType Directory -Force -Path $downloadRoot, $datasetRoot, $outputRoot, $resultRoot | Out-Null

if (-not $KeepOutputs) {
    Get-ChildItem -LiteralPath $outputRoot -File -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

if (-not (Test-Path -LiteralPath $fastzipExe)) {
    throw "FastZIP release executable not found: $fastzipExe. Run `cargo build --release` first."
}

$datasets = @(
    @{
        Name = 'canterbury'
        Group = 'small'
        Url = 'https://corpus.canterbury.ac.nz/resources/cantrbry.zip'
        DownloadPath = Join-Path $downloadRoot 'cantrbry.zip'
        ExtractPath = Join-Path $datasetRoot 'canterbury'
        Kind = 'zip'
    },
    @{
        Name = 'zlib-1.3.1'
        Group = 'small'
        Url = 'https://github.com/madler/zlib/archive/refs/tags/v1.3.1.zip'
        DownloadPath = Join-Path $downloadRoot 'zlib-1.3.1.zip'
        ExtractPath = Join-Path $datasetRoot 'zlib-1.3.1'
        Kind = 'zip'
    },
    @{
        Name = 'libpng-1.6.43'
        Group = 'small'
        Url = 'https://github.com/pnggroup/libpng/archive/refs/tags/v1.6.43.zip'
        DownloadPath = Join-Path $downloadRoot 'libpng-1.6.43.zip'
        ExtractPath = Join-Path $datasetRoot 'libpng-1.6.43'
        Kind = 'zip'
    },
    @{
        Name = 'sqlite-3.45.2'
        Group = 'medium'
        Url = 'https://github.com/sqlite/sqlite/archive/refs/tags/version-3.45.2.zip'
        DownloadPath = Join-Path $downloadRoot 'sqlite-3.45.2.zip'
        ExtractPath = Join-Path $datasetRoot 'sqlite-3.45.2'
        Kind = 'zip'
    },
    @{
        Name = 'python-3.12.0'
        Group = 'medium'
        Url = 'https://www.python.org/ftp/python/3.12.0/Python-3.12.0.tgz'
        DownloadPath = Join-Path $downloadRoot 'Python-3.12.0.tgz'
        ExtractPath = Join-Path $datasetRoot 'python-3.12.0'
        Kind = 'tar'
    },
    @{
        Name = 'linux-6.8'
        Group = 'large'
        Url = 'https://github.com/torvalds/linux/archive/refs/tags/v6.8.zip'
        DownloadPath = Join-Path $downloadRoot 'linux-6.8.zip'
        ExtractPath = Join-Path $datasetRoot 'linux-6.8'
        Kind = 'zip'
    }
)

function Get-InputBytes {
    param([string]$Path)

    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -File | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0 }
    return [uint64]$sum
}

function Get-IterationsForGroup {
    param([string]$Group)

    if ($Iterations -gt 0) {
        return $Iterations
    }

    switch ($Group) {
        'small' { return $SmallIterations }
        'medium' { return $MediumIterations }
        'large' { return $LargeIterations }
        default { throw "Unknown dataset group: $Group" }
    }
}

function Resolve-DatasetSourcePath {
    param([hashtable]$Dataset)

    $children = Get-ChildItem -LiteralPath $Dataset.ExtractPath -Force
    $directories = @($children | Where-Object { $_.PSIsContainer })
    $files = @($children | Where-Object { -not $_.PSIsContainer })

    if ($directories.Count -eq 1 -and $files.Count -eq 0) {
        return $directories[0].FullName
    }
    return $Dataset.ExtractPath
}

function Expand-Dataset {
    param([hashtable]$Dataset)

    if (Test-Path -LiteralPath $Dataset.ExtractPath) {
        $Dataset.SourcePath = Resolve-DatasetSourcePath $Dataset
        if (Test-Path -LiteralPath $Dataset.SourcePath) {
            return
        }
    }

    if (-not (Test-Path -LiteralPath $Dataset.DownloadPath)) {
        Write-Host "[zip_perf] downloading $($Dataset.Name) from $($Dataset.Url)"
        Invoke-WebRequest -Uri $Dataset.Url -OutFile $Dataset.DownloadPath
    }

    if (Test-Path -LiteralPath $Dataset.ExtractPath) {
        Remove-Item -LiteralPath $Dataset.ExtractPath -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $Dataset.ExtractPath | Out-Null

    switch ($Dataset.Kind) {
        'zip' {
            Expand-Archive -LiteralPath $Dataset.DownloadPath -DestinationPath $Dataset.ExtractPath -Force
        }
        'tar' {
            & tar.exe -xf $Dataset.DownloadPath -C $Dataset.ExtractPath
            if ($LASTEXITCODE -ne 0) {
                throw "Failed to extract $($Dataset.DownloadPath)"
            }
        }
        default {
            throw "Unsupported dataset kind: $($Dataset.Kind)"
        }
    }

    $Dataset.SourcePath = Resolve-DatasetSourcePath $Dataset
}

function Invoke-FastZipRun {
    param(
        [string]$InputPath,
        [string]$ArchivePath
    )

    & $fastzipExe compress $InputPath -o $ArchivePath --threads $Threads --level $Level --method deflate | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "FastZIP exited with code $LASTEXITCODE"
    }
}

function Invoke-BandizipRun {
    param(
        [string]$InputPath,
        [string]$ArchivePath
    )

    if (-not (Test-Path -LiteralPath $bandizipExe)) {
        throw "Bandizip CLI not found: $bandizipExe"
    }

    $bandizipLevel = switch ($Level) {
        'fastest' { 1 }
        'fast' { 3 }
        'normal' { 5 }
        'maximum' { 7 }
        'ultra' { 9 }
    }

    & $bandizipExe c -y -r -fmt:zip "-l:$bandizipLevel" $ArchivePath $InputPath | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Bandizip exited with code $LASTEXITCODE"
    }
}

function Invoke-7ZipRun {
    param(
        [string]$InputPath,
        [string]$ArchivePath
    )

    if (-not (Test-Path -LiteralPath $sevenZipExe)) {
        throw "7-Zip CLI not found: $sevenZipExe"
    }

    $sevenZipLevel = switch ($Level) {
        'fastest' { 1 }
        'fast' { 3 }
        'normal' { 5 }
        'maximum' { 7 }
        'ultra' { 9 }
    }

    & $sevenZipExe a -tzip "-mx=$sevenZipLevel" "-mmt=$Threads" -bd -y $ArchivePath $InputPath | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "7-Zip exited with code $LASTEXITCODE"
    }
}

function Invoke-TimedCompression {
    param(
        [string]$Tool,
        [hashtable]$Dataset,
        [int]$Iteration,
        [scriptblock]$Action
    )

    $archivePath = Join-Path $outputRoot "$Tool-$($Dataset.Name)-$Iteration.zip"
    if (Test-Path -LiteralPath $archivePath) {
        Remove-Item -LiteralPath $archivePath -Force
    }

    Write-Host "[zip_perf] start: tool=$Tool dataset=$($Dataset.Name) group=$($Dataset.Group) run=$Iteration"

    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
    [GC]::Collect()

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & $Action $Dataset.SourcePath $archivePath
    $stopwatch.Stop()

    $inputBytes = Get-InputBytes $Dataset.SourcePath
    $outputBytes = (Get-Item -LiteralPath $archivePath).Length
    $record = [pscustomobject]@{
        tool = $Tool
        group = $Dataset.Group
        dataset = $Dataset.Name
        iteration = $Iteration
        seconds = [Math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
        input_bytes = $inputBytes
        output_bytes = [uint64]$outputBytes
        throughput_mib_s = if ($stopwatch.Elapsed.TotalSeconds -gt 0) {
            [Math]::Round(($inputBytes / 1MB) / $stopwatch.Elapsed.TotalSeconds, 2)
        } else {
            0
        }
        ratio = if ($inputBytes -gt 0) {
            [Math]::Round($outputBytes / [double]$inputBytes, 4)
        } else {
            0
        }
    }

    Write-Host "[zip_perf] done:  tool=$Tool dataset=$($Dataset.Name) group=$($Dataset.Group) run=$Iteration seconds=$($record.seconds) ratio=$($record.ratio) throughput_mib_s=$($record.throughput_mib_s)"

    if (-not $KeepOutputs -and (Test-Path -LiteralPath $archivePath)) {
        Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
    }

    return $record
}

$selectedDatasets = $datasets | Where-Object { $Groups -contains $_.Group }
if ($selectedDatasets.Count -eq 0) {
    throw "No datasets matched the requested groups: $($Groups -join ', ')"
}

Write-Host "[zip_perf] fastzip executable: $fastzipExe"
Write-Host "[zip_perf] groups: $($Groups -join ', ')"
Write-Host "[zip_perf] level: $Level"
Write-Host "[zip_perf] threads: $Threads"
Write-Host "[zip_perf] compare bandizip: $(-not $SkipBandizip)"
Write-Host "[zip_perf] compare 7zip: $(-not $Skip7Zip)"
Write-Host "[zip_perf] keep outputs: $KeepOutputs"

$records = New-Object System.Collections.Generic.List[object]

foreach ($dataset in $selectedDatasets) {
    Expand-Dataset $dataset
    $runCount = Get-IterationsForGroup $dataset.Group
    $inputBytes = Get-InputBytes $dataset.SourcePath
    Write-Host "[zip_perf] dataset ready: $($dataset.Name) [$($dataset.Group)] $([Math]::Round($inputBytes / 1MB, 2)) MiB, runs=$runCount"

    for ($iteration = 1; $iteration -le $runCount; $iteration++) {
        $records.Add((Invoke-TimedCompression -Tool 'fastzip' -Dataset $dataset -Iteration $iteration -Action ${function:Invoke-FastZipRun}))
        if (-not $SkipBandizip) {
            $records.Add((Invoke-TimedCompression -Tool 'bandizip' -Dataset $dataset -Iteration $iteration -Action ${function:Invoke-BandizipRun}))
        }
        if (-not $Skip7Zip) {
            $records.Add((Invoke-TimedCompression -Tool '7zip' -Dataset $dataset -Iteration $iteration -Action ${function:Invoke-7ZipRun}))
        }
    }
}

$timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$groupTag = ($Groups -join '_')
$jsonPath = Join-Path $resultRoot "zip_perf_${groupTag}_$timestamp.json"
$csvPath = Join-Path $resultRoot "zip_perf_${groupTag}_$timestamp.csv"
$records | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $jsonPath -Encoding utf8
$records | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding utf8

$summary = $records |
    Group-Object tool, group, dataset |
    ForEach-Object {
        $items = $_.Group
        [pscustomobject]@{
            tool = $items[0].tool
            group = $items[0].group
            dataset = $items[0].dataset
            runs = $items.Count
            avg_seconds = [Math]::Round((($items | Measure-Object -Property seconds -Average).Average), 3)
            avg_throughput_mib_s = [Math]::Round((($items | Measure-Object -Property throughput_mib_s -Average).Average), 2)
            avg_ratio = [Math]::Round((($items | Measure-Object -Property ratio -Average).Average), 4)
        }
    } |
    Sort-Object group, dataset, tool

$summary | Format-Table -AutoSize
Write-Host
Write-Host "[zip_perf] json: $jsonPath"
Write-Host "[zip_perf] csv:  $csvPath"
