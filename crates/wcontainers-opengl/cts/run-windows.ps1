# Runs the case lists of lists.txt (or those named) through deqp-runner on the Windows desktop, with
# Mesa's opengl32.dll beside glcts and GALLIUM_DRIVER choosing the Gallium driver, and prints for each
# list the count of every result and the measure of plan 15.8: Pass and Warn over the cases run (all
# but NotSupported, which deqp-runner reports as Skip). Every Nth case from the Kth is a shard. As a
# GPU job, from a checkout on the Mac (glcts makes windows, so on the rig's desktop):
#
#   tools/graphics-rig/gpu-job --desktop -- powershell -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\run-windows.ps1 -Driver llvmpipe -Fraction 100
#
# Results in $Root\cts\results\<driver>\<list>: deqp-runner's results.csv, failures.csv and the logs.
param(
    [Parameter(Mandatory = $true)][string]$Driver,
    [string[]]$Lists = @(),
    [int]$Fraction = 1,
    [int]$FractionStart = 1,
    [int]$RunnerJobs = 0
)
. (Join-Path $PSScriptRoot '..\..\wcontainers-nir\mesa\windows-tools.ps1')
if ($RunnerJobs -le 0) { $RunnerJobs = $Jobs }

$Cts = Join-Path $Root 'cts'
$mustpass = Join-Path $Cts 'src\external\openglcts\data\gl_cts\data\mustpass'
$glctsDir = Join-Path $Cts 'build\external\openglcts\modules'
$glcts = Join-Path $glctsDir 'glcts.exe'
$runner = Join-Path $Tools 'deqp-runner\bin\deqp-runner.exe'
$mesaBin = Join-Path $Root 'mesa\install\bin'
foreach ($needed in $glcts, $runner) {
    if (-not (Test-Path $needed)) { throw "cts run-windows: $needed is missing; run cts\build-windows.ps1" }
}
foreach ($library in Get-ChildItem (Join-Path $mesaBin '*.dll')) {
    Copy-Item -Force $library.FullName $glctsDir
}

$all = Get-Content (Join-Path $PSScriptRoot 'lists.txt') | Where-Object { $_ -and -not $_.StartsWith('#') }
$chosen = if ($Lists) { $all | Where-Object { $Lists -contains [IO.Path]::GetFileNameWithoutExtension($_) } } else { $all }
if (-not $chosen) { throw "cts run-windows: none of $Lists is in lists.txt" }

$env:GALLIUM_DRIVER = $Driver
$failed = $false
foreach ($list in $chosen) {
    $caselist = Join-Path $mustpass $list
    $name = [IO.Path]::GetFileNameWithoutExtension($list)
    # The list's first configuration in its folder's mustpass.xml.
    [xml]$described = Get-Content (Join-Path (Split-Path $caselist) 'mustpass.xml')
    $configuration = $described.SelectNodes('//Configuration') |
        Where-Object { $_.caseListFile -eq (Split-Path -Leaf $list) } | Select-Object -First 1
    if (-not $configuration) { throw "cts run-windows: mustpass.xml has no configuration for $list" }
    $output = Join-Path $Cts "results\$Driver\$name"
    if (Test-Path $output) { Remove-Item -Recurse -Force $output }
    $deqpArguments = $configuration.commandLine -split ' ' | Where-Object { $_ }
    Write-Output "cts: $name on $Driver, every ${Fraction}th case from $FractionStart, $RunnerJobs at once: $($configuration.commandLine)"
    & $runner run --deqp $glcts --caselist $caselist --output $output --fraction $Fraction `
        --fraction-start $FractionStart --jobs $RunnerJobs -- @deqpArguments | Select-Object -Last 3
    if ($LASTEXITCODE -ne 0) { $failed = $true }

    $results = Join-Path $output 'results.csv'
    if (-not (Test-Path $results)) {
        Write-Output "cts: $name on ${Driver}: no results (deqp-runner could not run the list, above)"
        continue
    }
    $counts = @{}
    foreach ($row in Import-Csv -Header Case, Result, Seconds $results) {
        $counts[$row.Result] = 1 + [int]$counts[$row.Result]
    }
    $total = ($counts.Values | Measure-Object -Sum).Sum
    $run = $total - [int]$counts['Skip']
    $passed = [int]$counts['Pass'] + [int]$counts['Warn']
    $share = if ($run) { '{0:P2}' -f ($passed / $run) } else { 'none run' }
    $each = ($counts.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name) $($_.Value)" }) -join ', '
    Write-Output "cts: $name on ${Driver}: $total cases ($each); passed $passed of $run run, $share"
}
if ($failed) { exit 1 }
