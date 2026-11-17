# Runs Wine's Direct3D tests built by wine-tests.ps1, as a GPU job on the hidden desktop: each test
# of each chosen file in its own process (so one crash or hang loses only that test), on the GPU or
# on WARP, with replacement libraries beside the test programs if given. Writes <Out>\<file>-<driver>.txt,
# one line per test: "<test> <pass|fail|crash|timeout> <checks run> <checks failed> <skipped> <seconds>",
# and keeps each test's output in <Out>\logs. Without -Record, compares every result with the
# baseline recorded on real Direct3D (<Baseline>\<file>-<driver>.txt): a test that passed there must
# pass here; a test that did not is reported but not counted. Exits 1 if any counted test fails.
param(
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][string]$Baseline,
    [switch]$Warp,
    [switch]$Record,
    # Files to run, comma separated, each optionally with chosen tests: d3d11:test_a/test_b,dxgi
    [string]$Tests = 'd3d11,d3d10core,dxgi,d3d12',
    # Libraries put beside the test programs, as name=path pairs separated by semicolons.
    [string]$Dlls = '',
    [int]$Limit = 120
)
$ErrorActionPreference = 'Stop'
trap { [Console]::Error.WriteLine("wine-run: $_"); exit 2 }
$driver = if ($Warp) { 'warp' } else { 'hardware' }
$bin = "$Out\bin"
foreach ($dir in $Out, $bin, "$Out\logs") { New-Item -ItemType Directory -Force $dir | Out-Null }
Get-ChildItem $bin | Remove-Item -Force
Get-ChildItem "$Out\*.txt" | Remove-Item -Force
foreach ($pair in ($Dlls.Split(';') | Where-Object { $_ })) {
    $name, $path = $pair.Split('=', 2)
    Copy-Item $path "$bin\$name"
}
$excluded = @{}
foreach ($line in Get-Content tools\graphics-rig\wine-excluded.txt) {
    if ($line -match '^(\w+) (\w+): (.*)$') { $excluded["$($Matches[1]) $($Matches[2])"] = $Matches[3] }
}

$counted = 0; $failed = 0
foreach ($choice in $Tests.Split(',')) {
    $file, $chosen = $choice.Split(':', 2)
    $exe = "C:\wc\wine\build\${file}_test.exe"
    if (-not (Test-Path $exe)) { throw "no $exe; run wine-tests build first" }
    Copy-Item $exe $bin
    $all = Get-Content "C:\wc\wine\build\$file.tests"
    $names = if ($chosen) { $chosen.Split('/') } else { $all }
    foreach ($name in $names) { if ($all -notcontains $name) { throw "$file has no test $name; see C:\wc\wine\build\$file.tests" } }
    $expected = @{}
    $baselineFile = "$Baseline\$file-$driver.txt"
    if (-not $Record) {
        if (-not (Test-Path $baselineFile)) { throw "no baseline $baselineFile; record one with wine-tests run --record" }
        foreach ($line in Get-Content $baselineFile) { $fields = $line.Split(' '); $expected[$fields[0]] = $fields }
    }
    $results = New-Object System.Collections.Generic.List[string]
    $fileClock = [Diagnostics.Stopwatch]::StartNew()
    foreach ($name in $names) {
        if ($excluded.ContainsKey("$file $name")) { "$file ${name}: excluded, $($excluded["$file $name"])"; continue }
        $log = "$Out\logs\$file-$name-$driver.log"
        $info = New-Object Diagnostics.ProcessStartInfo "$bin\${file}_test.exe", ($file + $(if ($Warp) { ' --warp' } else { '' }))
        $info.UseShellExecute = $false
        $info.WorkingDirectory = $bin
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        $info.EnvironmentVariables['WC_WINE_ONLY'] = $name
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $process = [Diagnostics.Process]::Start($info)
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $verdict = $null
        if (-not $process.WaitForExit($Limit * 1000)) {
            & taskkill.exe /T /F /PID $process.Id 2>&1 | Out-Null
            $process.WaitForExit()
            $verdict = 'timeout'
        }
        $seconds = [math]::Round($clock.Elapsed.TotalSeconds, 2)
        $text = $stdout.Result + $stderr.Result
        [IO.File]::WriteAllText($log, $text)
        $summary = [regex]::Match($text, '(\d+) tests executed \((\d+) marked as todo, (\d+) as flaky, (\d+) failures?\), (\d+) skipped')
        $checks = 0; $failures = 0; $skipped = 0
        if ($summary.Success) {
            $checks = [int]$summary.Groups[1].Value; $failures = [int]$summary.Groups[4].Value
            $skipped = [int]$summary.Groups[5].Value
        }
        if (-not $verdict) {
            $verdict = if (-not $summary.Success -or $process.ExitCode -gt 255 -or $process.ExitCode -lt 0) { 'crash' }
                       elseif ($failures -eq 0 -and $process.ExitCode -eq 0) { 'pass' } else { 'fail' }
        }
        $results.Add("$name $verdict $checks $failures $skipped $seconds")
        $what = "$verdict, $checks checks, $failures failed, $skipped skipped, $seconds s"
        if ($verdict -eq 'crash') { $what += (', exit 0x{0:x8}' -f $process.ExitCode) }
        if ($Record) { "$file ${name}: $what"; continue }
        $before = $expected[$name]
        if (-not $before) { "$file ${name}: FAIL, not in the baseline; $what"; $failed++; $counted++ }
        elseif ($before[1] -ne 'pass') { "$file ${name}: known, $($before[1]) on real Direct3D ($($before[3]) of $($before[2]) checks failed); here $what" }
        elseif ($verdict -eq 'pass') { "$file ${name}: pass, $checks checks (real Direct3D $($before[2])), $skipped skipped (real Direct3D $($before[4])), $seconds s"; $counted++ }
        else { "$file ${name}: FAIL, passes on real Direct3D; here $what"; $failed++; $counted++ }
    }
    [IO.File]::WriteAllLines("$Out\$file-$driver.txt", [string[]]$results)
    $passed = @($results | Where-Object { $_.Split(' ')[1] -eq 'pass' }).Count
    "$file on ${driver}: $($results.Count) tests run, $passed passed, $([math]::Round($fileClock.Elapsed.TotalSeconds, 1)) s"
}
if (-not $Record) {
    "$counted tests counted (passing on real Direct3D), $failed FAIL"
    if ($failed) { exit 1 }
}
