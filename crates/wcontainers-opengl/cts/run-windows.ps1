# Runs the case lists of lists.txt (or those named) through deqp-runner on the Windows desktop, with
# Mesa's opengl32.dll beside glcts and GALLIUM_DRIVER choosing the Gallium driver (or, with -Driver
# system, the system's OpenGL), and prints for each
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
. (Join-Path $PSScriptRoot 'cts-windows.ps1')
if ($RunnerJobs -le 0) { $RunnerJobs = $Jobs }
if (-not (Test-Path $Runner)) { throw "cts run-windows: $Runner is missing; run cts\build-windows.ps1" }
$program = New-DriverFolder $Driver

# `powershell -File` hands a comma list over as one string.
$Lists = $Lists | ForEach-Object { $_ -split ',' } | Where-Object { $_ }
$all = Get-Lists
$chosen = if ($Lists) { $all | Where-Object { $Lists -contains [IO.Path]::GetFileNameWithoutExtension($_) } } else { $all }
if (-not $chosen) { throw "cts run-windows: none of $Lists is in lists.txt" }

$failed = $false
foreach ($list in $chosen) {
    $caselist = Join-Path $Mustpass $list
    $name = [IO.Path]::GetFileNameWithoutExtension($list)
    $output = Join-Path $Cts "results\$Driver\$name"
    if (Test-Path $output) { Remove-Item -Recurse -Force $output }
    $deqpArguments = Get-ListArguments $list
    Write-Output "cts: $name on $Driver, every ${Fraction}th case from $FractionStart, $RunnerJobs at once: $deqpArguments"
    & $Runner run --deqp $program --caselist $caselist --output $output --fraction $Fraction `
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
