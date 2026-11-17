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

# An alone job pauses the default group and resumes it when it ends; if its wrapper was killed
# first, the group stays paused, so it is resumed here when no alone job is queued or running.
$state = & $pueue -c $config status --json | ConvertFrom-Json
$aloneTasks = @($state.tasks.PSObject.Properties.Value | Where-Object { $_.group -eq 'alone' -and
        ($_.status.PSObject.Properties.Name | Where-Object { $_ -in 'Queued', 'Running', 'Stashed', 'Locked' }) })
if ($state.groups.default.status -ne 'Running' -and $aloneTasks.Count -eq 0) {
    [Console]::Error.WriteLine('gpu-job: resuming the default group, paused with no alone job')
    & $pueue -c $config start --group default | Out-Null
}

# The job runs with this environment, which pueue records when the job is added.
. $PSScriptRoot\env.ps1
$env:CARGO_TARGET_DIR = "C:\wc\target\$Name"
$text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Command))
$label = "$Name`: $text"
if ($label.Length -gt 120) { $label = $label.Substring(0, 120) }
$group = if ($Alone) { 'alone' } else { 'default' }
$arguments = @('-c', $config, 'add', '--print-task-id', '--group', $group, "--priority=$Priority",
    '--working-directory', $checkout, '--label', $label, '--',
    'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', 'C:\wc\rig\job.ps1',
    '-Name', $Name, '-Tree', $Tree)
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
