# Runs Wine's and vkd3d's Direct3D tests built by wine-tests.ps1, as a GPU job on the hidden desktop:
# each test of each chosen file in its own process (so one crash or hang loses only that test; for
# vkd3d's shader runner a test is a .shader_test file, named by its path under vkd3d's tests with +
# for /), on the GPU or
# on WARP, with replacement libraries beside the test programs if given. Writes <Out>\<file>-<driver>.txt,
# one line per test: "<test> <pass|fail|crash|timeout> <checks run> <checks failed> <skipped> <seconds>
# [unstable]", and <Out>\<file>-<driver>.failures, the checks each test failed: "<test>
# <file>:<line> <evaluations>", which evaluations of that place in the source failed (3,5,7-40). It
# keeps each test's output in <Out>\logs. Recording runs a test that does not pass twice more with
# every check reported: one that fails other evaluations, or runs another number of checks, the
# second time is marked unstable. Without -Record, compares every result with the baseline recorded
# on real Direct3D (<Baseline>\<file>-<driver>.txt and .failures): a test must run at least as many
# checks and skip no more (a library that refuses to make a device skips nearly everything with no
# failure), and a test real Direct3D does not pass runs with every check reported and may fail only
# the evaluations real Direct3D fails, and when real Direct3D crashed in it, must run as many checks
# before crashing; only an unstable one is reported but not counted. Exits 1 if any counted test
# fails. Each test runs through gt-launch (-Launch), which reports where it loaded each replacement
# from: a test that loaded one from anywhere else fails. On the GPU, a test quarantined on the desktop
# (C:\wc\rig\hazard.ps1, as "wine <file>:<test>") does not run, and a display driver reset while a
# test runs quarantines that test and ends the run.
param(
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][string]$Baseline,
    [switch]$Warp,
    [switch]$Record,
    # Files to run, comma separated, each optionally with chosen tests: d3d11:test_a/test_b,dxgi
    [string]$Tests = 'd3d11,d3d10core,dxgi,d3d12,vkd3d-d3d12,vkd3d-d3d12_invalid_usage,vkd3d-hlsl_d3d12,vkd3d-shader_runner',
    # Libraries put beside the test programs, as name=path pairs separated by semicolons.
    [string]$Dlls = '',
    [int]$Limit = 120,
    [Parameter(Mandatory)][string]$Launch
)
$ErrorActionPreference = 'Stop'
trap { [Console]::Error.WriteLine("wine-run: $_"); exit 2 }
$driver = if ($Warp) { 'warp' } else { 'hardware' }
$bin = "$Out\bin"
foreach ($dir in $Out, $bin, "$Out\logs") { New-Item -ItemType Directory -Force $dir | Out-Null }
Get-ChildItem $bin | Remove-Item -Force
Get-ChildItem "$Out\*.txt", "$Out\*.failures" | Remove-Item -Force
# The SDK's DXIL compiler and validator, which vkd3d's tests load to compile shader model 6 shaders.
Copy-Item "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.26100.0\x64\dxcompiler.dll", "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.26100.0\x64\dxil.dll" $bin
$replacements = @()
foreach ($pair in ($Dlls.Split(';') | Where-Object { $_ })) {
    $name, $path = $pair.Split('=', 2)
    Copy-Item $path "$bin\$name"
    $replacements += $name.ToLower()
}
$wanted = if ($replacements) { $replacements -join ',' } else { '-' }
$loadedBy = @{}
foreach ($name in $replacements) { $loadedBy[$name] = 0 }
$launched = 0
. C:\wc\rig\hazard.ps1
$quarantine = Get-Quarantine
# The tests run on the GPU so far, with their times, to find the one running at a reset.
$ran = New-Object System.Collections.Generic.List[object]
$checkedTo = Get-Date
# Stops the run if the display driver was reset or logged errors since the last look, quarantining
# the test that was running then (or that had just ended: the driver logs a second or two late).
function Check-Resets {
    if ($Warp) { return }
    $now = Get-Date
    $events = Get-DisplayEvents $script:checkedTo $now.AddSeconds(2)
    $script:checkedTo = $now
    if (-not $events) { return }
    $at = $events[0].TimeCreated
    $suspect = $ran | Where-Object { $_.Start -le $at -and $at -le $_.End.AddSeconds(3) } | Select-Object -Last 1
    $lines = $events | ForEach-Object { Format-DisplayEvent $_ }
    if ($suspect) {
        Add-Quarantine "wine $($suspect.Name)" "display driver reset at $($lines[0].Substring(0, 19)) while it ran, in $Out"
        # The job or remote command running this quarantines the test in place of its own command.
        if ($env:WC_NARROWED) { Add-HazardLines $env:WC_NARROWED @("wine $($suspect.Name)") }
        "$($suspect.Name): FAIL, the display driver was reset while it ran; it is now quarantined on the GPU and the run stops:"
    } else {
        "FAIL, the display driver was reset during the run, not while a test ran; the run stops:"
    }
    $lines | ForEach-Object { "  $_" }
    exit 1
}
# The excluded tests; one marked "(gpu)" runs on WARP.
$excluded = @{}
foreach ($line in Get-Content tools\graphics-rig\wine-excluded.txt) {
    if ($line -match '^(\w+) (\w+)( \(gpu\))?: (.*)$' -and -not ($Matches[3] -and $Warp)) { $excluded["$($Matches[1]) $($Matches[2])"] = $Matches[4] }
}

# Runs one test in its own process through gt-launch. Returns its verdict, counts, the checks it
# failed and where it loaded the replacements from. A check is its place in the source, file:line,
# and which evaluation of that place it was ("d3d11.c:120#3", counting from 1): with $Every the
# test reports every check it passes as well (WINETEST_REPORT_SUCCESS, VKD3D_TEST_DEBUG=2), so the
# number is the evaluation's own and a crash still shows how many checks ran; without, only failed
# checks are numbered.
function Run-Test([string]$file, [string]$name, [bool]$vkd3d, [bool]$Every) {
    $log = "$Out\logs\$file-$name-$driver.log"
    # Wine's tests take the file's name and pick tests by WC_WINE_ONLY; vkd3d's by
    # VKD3D_TEST_FILTER, and its shader runner takes the .shader_test file.
    $arguments = @(if ($vkd3d) { } else { $file }) + @(if ($Warp) { '--warp' })
    if ($file -eq 'vkd3d-shader_runner') { $arguments += "C:\wc\wine\vkd3d\tests\$($name.Replace('+', '\'))" }
    $report = "$Out\logs\$file-$name-$driver.loaded"
    $info = New-Object Diagnostics.ProcessStartInfo $Launch, ("`"$report`" $wanted `"$bin\${file}_test.exe`" " + ($arguments -join ' '))
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $bin
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    if (-not $vkd3d) { $info.EnvironmentVariables['WC_WINE_ONLY'] = $name }
    elseif ($file -ne 'vkd3d-shader_runner') { $info.EnvironmentVariables['VKD3D_TEST_FILTER'] = $name }
    if ($Every) {
        if ($vkd3d) { $info.EnvironmentVariables['VKD3D_TEST_DEBUG'] = '2' } else { $info.EnvironmentVariables['WINETEST_REPORT_SUCCESS'] = '1' }
    }
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
    # Wine: "<n> tests executed (<t> marked as todo, <f> as flaky, <n> failures), <n> skipped";
    # vkd3d: "<n> tests executed (<n> failures, <n> skipped, <n> todo, <n> bugs)".
    $checks = 0; $failures = 0; $skipped = 0
    if ($vkd3d) {
        $summary = [regex]::Match($text, '(\d+) tests executed \((\d+) failures, (\d+) skipped, \d+ todo, \d+ bugs\)')
        if ($summary.Success) { $failures = [int]$summary.Groups[2].Value; $skipped = [int]$summary.Groups[3].Value }
    } else {
        $summary = [regex]::Match($text, '(\d+) tests executed \((\d+) marked as todo, (\d+) as flaky, (\d+) failures?\), (\d+) skipped')
        if ($summary.Success) { $failures = [int]$summary.Groups[4].Value; $skipped = [int]$summary.Groups[5].Value }
    }
    # Each check printed: Wine's "<file>:<line>: [<context>: ]Test <outcome>", vkd3d's the same.
    $seen = @{}; $evaluated = 0
    $checksFailed = [ordered]@{}
    foreach ($match in [regex]::Matches($text, '(?m)^([\w.+-]+:\d+):.*?Test (failed|succeeded inside todo block|succeeded|marked todo|marked flaky)[^\r\n]*')) {
        $at = $match.Groups[1].Value
        $seen[$at] = 1 + $(if ($seen.ContainsKey($at)) { $seen[$at] } else { 0 })
        $evaluated++
        if ($match.Groups[2].Value -in 'failed', 'succeeded inside todo block') {
            $checksFailed["$at#$($seen[$at])"] = $match.Value
        }
    }
    if ($summary.Success) { $checks = [int]$summary.Groups[1].Value } elseif ($Every) { $checks = $evaluated }
    if (-not $verdict) {
        $verdict = if (-not $summary.Success -or $process.ExitCode -gt 255 -or $process.ExitCode -lt 0) { 'crash' }
                   elseif ($failures -eq 0 -and $process.ExitCode -eq 0) { 'pass' } else { 'fail' }
    }
    # Where the test loaded each replacement from, if it loaded it at all.
    $here = @(); $elsewhere = @()
    if ($replacements -and (Test-Path $report)) {
        foreach ($line in Get-Content $report) {
            $library, $from = $line -split ' from ', 2
            if ($from -eq 'not loaded') { continue }
            if ($from -ieq "$bin\$library") { $here += $library } else { $elsewhere += "$library from $from" }
        }
    }
    @{ Verdict = $verdict; Checks = $checks; Failures = $failures; Skipped = $skipped; Seconds = $seconds; Exit = $process.ExitCode; Failed = $checksFailed; Here = @($here | Select-Object -Unique); Elsewhere = $elsewhere }
}

# A test's failed checks as "<file>:<line> <evaluations>" lines, the evaluations as ranges (3,5,7-40).
function Failure-Lines($failed) {
    $byPlace = @{}
    foreach ($key in $failed.Keys) {
        $at, $n = $key.Split('#')
        if (-not $byPlace.ContainsKey($at)) { $byPlace[$at] = New-Object System.Collections.Generic.List[int] }
        $byPlace[$at].Add([int]$n)
    }
    @($byPlace.Keys | Sort-Object | ForEach-Object {
        $runs = @(); $numbers = @($byPlace[$_] | Sort-Object)
        for ($i = 0; $i -lt $numbers.Count; $i++) {
            $from = $numbers[$i]
            while ($i + 1 -lt $numbers.Count -and $numbers[$i + 1] -eq $numbers[$i] + 1) { $i++ }
            $runs += $(if ($numbers[$i] -eq $from) { "$from" } else { "$from-$($numbers[$i])" })
        }
        "$_ $($runs -join ',')"
    })
}

# The checks a failures line allows to fail, as Run-Test names them.
function Failure-Keys([string]$at, [string]$ranges) {
    foreach ($range in $ranges.Split(',')) {
        $from, $to = $range.Split('-')
        foreach ($n in [int]$from..[int]$(if ($to) { $to } else { $from })) { "$at#$n" }
    }
}

function Describe($run) {
    $what = "$($run.Verdict), $($run.Checks) checks, $($run.Failures) failed, $($run.Skipped) skipped, $($run.Seconds) s"
    if ($run.Verdict -eq 'crash') { $what += (', exit 0x{0:x8}' -f $run.Exit) }
    $what
}

$counted = 0; $failed = 0
foreach ($choice in $Tests.Split(',')) {
    $file, $chosen = $choice.Split(':', 2)
    $vkd3d = $file.StartsWith('vkd3d-')
    $exe = "C:\wc\wine\build\${file}_test.exe"
    if (-not (Test-Path $exe)) { throw "no $exe; run wine-tests build first" }
    Copy-Item $exe $bin
    $all = Get-Content "C:\wc\wine\build\$file.tests"
    $names = if ($chosen) { $chosen.Split('/') } else { $all }
    foreach ($name in $names) { if ($all -notcontains $name) { throw "$file has no test $name; see C:\wc\wine\build\$file.tests" } }
    $expected = @{}
    $known = @{}
    $baselineFile = "$Baseline\$file-$driver.txt"
    if (-not $Record) {
        if (-not (Test-Path $baselineFile)) { throw "no baseline $baselineFile; record one with wine-tests run --record" }
        foreach ($line in Get-Content $baselineFile) { $fields = $line.Split(' '); $expected[$fields[0]] = $fields }
        $failuresFile = "$Baseline\$file-$driver.failures"
        if (-not (Test-Path $failuresFile)) { throw "no $failuresFile; record the baseline again with wine-tests run --record" }
        foreach ($line in Get-Content $failuresFile) {
            $test, $at, $ranges = $line.Split(' ')
            if (-not $ranges) { throw "$failuresFile has an old line ($line); record the baseline again with wine-tests run --record" }
            if (-not $known.ContainsKey($test)) { $known[$test] = New-Object 'System.Collections.Generic.HashSet[string]' }
            foreach ($key in Failure-Keys $at $ranges) { $null = $known[$test].Add($key) }
        }
    }
    $results = New-Object System.Collections.Generic.List[string]
    $failing = New-Object System.Collections.Generic.List[string]
    $filePassed = 0; $fileFailed = 0; $fileKnown = 0
    $fileClock = [Diagnostics.Stopwatch]::StartNew()
    foreach ($name in $names) {
        if ($excluded.ContainsKey("$file $name")) { "$file ${name}: excluded, $($excluded["$file $name"])"; continue }
        if (-not $Warp -and $quarantine.ContainsKey("wine ${file}:$name")) {
            "$file ${name}: not run on the GPU, quarantined ($($quarantine["wine ${file}:$name"].Split("`t")[1..2] -join ': '))"
            continue
        }
        $before = $expected[$name]
        # A test real Direct3D does not pass reports every check, so each failure is pinned to its evaluation.
        $testStart = Get-Date
        $run = Run-Test $file $name $vkd3d ($before -and $before[1] -ne 'pass')
        $ran.Add(@{ Name = "${file}:$name"; Start = $testStart; End = (Get-Date) })
        Check-Resets
        $launched++
        foreach ($library in $run.Here) { $loadedBy[$library]++ }
        if ($Record) {
            $unstable = ''
            $what = Describe $run
            if ($run.Verdict -ne 'pass') {
                # Twice more with every check reported: it must fail the same evaluations each time
                # to be held to the rest.
                $run = Run-Test $file $name $vkd3d $true
                $again = Run-Test $file $name $vkd3d $true
                $what = Describe $run
                $first = (Failure-Lines $run.Failed) -join "`n"
                $second = (Failure-Lines $again.Failed) -join "`n"
                if ($first -ne $second -or $again.Verdict -ne $run.Verdict -or $again.Checks -ne $run.Checks) {
                    $unstable = ' unstable'
                    $what += "; again $(Describe $again): unstable"
                }
            }
            $results.Add("$name $($run.Verdict) $($run.Checks) $($run.Failures) $($run.Skipped) $($run.Seconds)$unstable")
            foreach ($line in Failure-Lines $run.Failed) { $failing.Add("$name $line") }
            "$file ${name}: $what"
            continue
        }
        $what = Describe $run
        $results.Add("$name $($run.Verdict) $($run.Checks) $($run.Failures) $($run.Skipped) $($run.Seconds)")
        foreach ($line in Failure-Lines $run.Failed) { $failing.Add("$name $line") }
        $allowed = if ($known.ContainsKey($name)) { $known[$name] } else { @() }
        # Checks failed here at an evaluation real Direct3D passed.
        $worse = @($run.Failed.Keys | Where-Object { -not $allowed.Contains($_) })
        $real = if ($before) { "real Direct3D: $($before[1]), $($before[2]) checks, $($before[3]) failed, $($before[4]) skipped" }
        $line = if ($run.Elsewhere) { "FAIL, did not load the replacement: $($run.Elsewhere -join ', ')" }
                elseif (-not $before) { "FAIL, not in the baseline; $what" }
                elseif ($before.Count -gt 6 -and $before[6] -eq 'unstable') { "known, fails differently from run to run on real Direct3D ($real); here $what" }
                elseif ($worse) { "FAIL, $($worse.Count) checks fail that real Direct3D passes, the first ($($worse[0])): $($run.Failed[$worse[0]]); here $what ($real)" }
                elseif ($before[1] -eq 'pass' -and $run.Verdict -ne 'pass') { "FAIL, passes on real Direct3D; here $what" }
                elseif ($run.Verdict -in 'crash', 'timeout' -and $run.Verdict -ne $before[1]) { "FAIL, $what ($real)" }
                elseif ($run.Checks -lt [int]$before[2] -or $run.Skipped -gt [int]$before[4]) {
                    "FAIL, ran $($run.Checks) checks (real Direct3D $($before[2])) and skipped $($run.Skipped) (real Direct3D $($before[4])), $($run.Seconds) s"
                }
                elseif ($before[1] -ne 'pass') { "pass, failing only checks real Direct3D fails ($($run.Failures) of $($run.Checks); $real), $($run.Seconds) s" }
                else { "pass, $($run.Checks) checks (real Direct3D $($before[2])), $($run.Skipped) skipped (real Direct3D $($before[4])), $($run.Seconds) s" }
        "$file ${name}: $line"
        if ($line.StartsWith('known')) { $fileKnown++ } else { $counted++; if ($line.StartsWith('FAIL')) { $failed++; $fileFailed++ } else { $filePassed++ } }
    }
    if ($Record -and $chosen -and (Test-Path $baselineFile)) {
        # Recording chosen tests keeps the other tests' lines of the baseline, in the file's order.
        $order = @{}; for ($i = 0; $i -lt $all.Count; $i++) { $order[$all[$i]] = $i }
        $byTest = { $order[$_.Split(' ')[0]] }
        $results = @(@(Get-Content $baselineFile | Where-Object { $names -notcontains $_.Split(' ')[0] }) + $results | Sort-Object $byTest, { $_ })
        $failing = @(@(Get-Content "$Baseline\$file-$driver.failures" | Where-Object { $names -notcontains $_.Split(' ')[0] }) + $failing | Sort-Object $byTest, { $_ })
    }
    [IO.File]::WriteAllLines("$Out\$file-$driver.failures", [string[]]$failing)
    [IO.File]::WriteAllLines("$Out\$file-$driver.txt", [string[]]$results)
    $seconds = [math]::Round($fileClock.Elapsed.TotalSeconds, 1)
    if ($Record) {
        $passed = @($results | Where-Object { $_.Split(' ')[1] -eq 'pass' }).Count
        "$file on ${driver}: $($results.Count) tests run, $passed passed, $seconds s"
    } else {
        "$file on ${driver}: $($results.Count) tests run, $filePassed pass, $fileFailed FAIL, $fileKnown known (failing differently from run to run on real Direct3D, not counted), $seconds s"
    }
}
if (-not $Warp) { Start-Sleep -Seconds 2; Check-Resets }
foreach ($name in $replacements) { "replacement ${name}: loaded from $bin by $($loadedBy[$name]) of $launched tests run" }
if (-not $Record) {
    "$counted tests counted, $failed FAIL"
    if ($failed) { exit 1 }
}
