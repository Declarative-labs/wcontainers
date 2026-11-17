# Runs Wine's Direct3D tests built by wine-tests.ps1, as a GPU job: each named test file (d3d11,
# d3d10core, dxgi, d3d12) on the GPU or on WARP, its whole output kept in <Out>\<test>-<driver>.log.
# Prints each test's summary line, how many checks failed, and how long it took; exits 1 if any
# test failed or crashed.
param(
    [Parameter(Mandatory)][string]$Out,
    [switch]$Warp,
    # Comma-separated, as -File passes no arrays.
    [string]$Tests = 'd3d11,d3d10core,dxgi,d3d12'
)
$ErrorActionPreference = 'Stop'
$driver = if ($Warp) { 'warp' } else { 'hardware' }
New-Item -ItemType Directory -Force $Out | Out-Null
$failed = $false
foreach ($test in $Tests.Split(',')) {
    $exe = "C:\wc\wine\build\${test}_test.exe"
    if (-not (Test-Path $exe)) { throw "no $exe; run wine-tests build first" }
    $log = "$Out\$test-$driver.log"
    $arguments = @($test) + $(if ($Warp) { @('--warp') } else { @() })
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process $exe -ArgumentList $arguments -NoNewWindow -PassThru -RedirectStandardOutput $log -RedirectStandardError "$log.err"
    $null = $process.Handle
    $process.WaitForExit()
    $seconds = [math]::Round($clock.Elapsed.TotalSeconds, 1)
    Get-Content "$log.err" | Add-Content $log
    Remove-Item "$log.err"
    $summary = Select-String -Path $log -Pattern ': \d+ tests executed' | Select-Object -Last 1
    $failures = @(Select-String -Path $log -Pattern 'Test failed:').Count
    $line = if ($summary) { $summary.Line.Trim() } else { "no summary (exit code $($process.ExitCode))" }
    "$test on ${driver}: $line; $failures failed checks; exit $($process.ExitCode); $seconds s"
    if ($process.ExitCode -ne 0) { $failed = $true }
}
if ($failed) { exit 1 }
