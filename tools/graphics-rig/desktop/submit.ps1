# Runs on the GPU desktop for `gpu-job`: queues a job, waits for it, prints its standard output and
# error each to its own, and exits with its exit code.
param(
    [Parameter(Mandatory)][string]$Name,
    # The git tree id the checkout C:\wc\src\<Name> must hold when the job runs.
    [Parameter(Mandatory)][string]$Tree,
    # The build directory must also have been built from that tree since it was sent.
    [switch]$Built,
    [int]$Priority = 0,
    [switch]$Alone,
    # Accepted from older clients and ignored: the queue decides which jobs may run beside others
    # (Test-Cleared in hazard.ps1).
    [switch]$Reference,
    # Runs the command on the hidden desktop of the logged-in user's session (see job.ps1).
    [switch]$Desktop,
    # The command, base64 of its UTF-8 text, run by cmd.exe in C:\wc\src\<Name>.
    [Parameter(Mandatory)][string]$Command
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("gpu-job: $_"); exit 125 }
$pueue = 'C:\wc\bin\pueue.exe'
$config = 'C:\wc\pueue\pueue.yml'
$checkout = "C:\wc\src\$Name"
$errors = "C:\wc\pueue\errors\$([guid]::NewGuid()).log"
New-Item -ItemType Directory -Force C:\wc\pueue\errors | Out-Null
if (-not (Test-Path $checkout)) { throw "no checkout $checkout; send one with remote first" }
. $PSScriptRoot\hazard.ps1
$text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Command))
$quarantined = Find-Quarantine $text
if ($quarantined) { throw "not run: quarantined, as suspected of a display driver reset ($quarantined); see tools/graphics-rig/resets" }
if (Test-Held) { throw "not run: the queue is held after a display driver reset until its cause is recorded (tools/graphics-rig/resets): $((Get-Content $hazardHeld) -join ' / ')" }

# An alone job pauses the other groups and resumes them when it ends; if its wrapper was killed
# first, they stay paused, so they are resumed here when no alone job is queued or running.
Resume-Groups | ForEach-Object { [Console]::Error.WriteLine("gpu-job: $_, paused with no alone job and no hold") }

# The job runs with this environment, which pueue records when the job is added.
. $PSScriptRoot\env.ps1
$env:CARGO_TARGET_DIR = "C:\wc\target\$Name"
$label = "$Name`: $text"
if ($label.Length -gt 120) { $label = $label.Substring(0, 120) }
# The rig's own runs on rig code seen to run without a reset go beside others; every other job
# runs in the group hazard, one at a time, and a rig run that is not cleared yet clears its code.
$key = if (Test-RigRun $text) { Get-RigKey $checkout $Tree } else { '' }
$group = if ($Alone) { 'alone' } elseif (Test-Cleared $text $key) { 'default' } else { 'hazard' }
$arguments = @('-c', $config, 'add', '--print-task-id', '--group', $group, "--priority=$Priority",
    '--working-directory', $checkout, '--label', $label, '--',
    'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', 'C:\wc\rig\job.ps1',
    '-Name', $Name, '-Tree', $Tree, '-Group', $group)
if ($key) { $arguments += '-RigKey', $key }
if ($Built) { $arguments += '-Built' }
if ($Alone) { $arguments += '-Alone' }
if ($Desktop) { $arguments += '-Desktop' }
$arguments += '-ErrorFile', $errors, '-Command', $Command
# Windows PowerShell 5.1 passes arguments to programs without escaping their quotes, so the command
# line is made here by the rules programs parse it with.
$quoted = foreach ($argument in $arguments) {
    if ($argument -and $argument -notmatch '[\s"]') { $argument }
    else { '"' + ($argument -replace '(\\*)"', '$1$1\"' -replace '(\\+)$', '$1$1') + '"' }
}
# An alone job holds the other groups back from its submission on; its wrapper resumes them.
if ($Alone) { foreach ($other in 'default', 'hazard') { & $pueue -c $config pause --group $other --wait | Out-Null } }
$info = New-Object System.Diagnostics.ProcessStartInfo $pueue, ($quoted -join ' ')
$info.UseShellExecute = $false
$info.RedirectStandardOutput = $true
$add = [System.Diagnostics.Process]::Start($info)
$id = $add.StandardOutput.ReadToEnd().Trim()
$add.WaitForExit()
if ($add.ExitCode -ne 0 -or -not $id) { throw 'pueue add failed' }
[Console]::Error.WriteLine("gpu-job: task $id in group $group at priority $Priority")
& $pueue -c $config wait $id --quiet | Out-Null

$task = (& $pueue -c $config status --json | ConvertFrom-Json).tasks.$id
$log = "C:\wc\pueue\data\task_logs\$id.log"
if (Test-Path $log) { [Console]::Out.Write([IO.File]::ReadAllText($log)) }
if (Test-Path $errors) { [Console]::Error.Write([IO.File]::ReadAllText($errors)); Remove-Item $errors }
$result = $task.status.Done.result
$start = [datetime]$task.status.Done.start
$enqueued = [datetime]$task.status.Done.enqueued_at
$end = [datetime]$task.status.Done.end
[Console]::Error.WriteLine(("gpu-job: task $id queued {0:HH:mm:ss.fff}, started {1:HH:mm:ss.fff}, ended {2:HH:mm:ss.fff}: {3}" -f $enqueued, $start, $end, ($result | ConvertTo-Json -Compress)))
& $pueue -c $config remove $id | Out-Null
if ($result -eq 'Success') { exit 0 }
if ($null -ne $result.Failed) { exit $result.Failed }
exit 124
