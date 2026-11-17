# Runs on the GPU desktop for `gpu-job`: queues a job, waits for it, prints its output and exits
# with its exit code.
param(
    [Parameter(Mandatory)][string]$Name,
    [int]$Priority = 0,
    [switch]$Alone,
    # The command, base64 of its UTF-8 text, run by cmd.exe in C:\wc\src\<Name>.
    [Parameter(Mandatory)][string]$Command
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("gpu-job: $_"); exit 125 }
$pueue = 'C:\wc\bin\pueue.exe'
$config = 'C:\wc\pueue\pueue.yml'
$checkout = "C:\wc\src\$Name"
if (-not (Test-Path $checkout)) { throw "no checkout $checkout; send one with remote first" }

# The job runs with this environment, which pueue records when the job is added.
$env:CARGO_TARGET_DIR = "C:\wc\target\$Name"
$text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Command))
$label = "$Name`: $text"
if ($label.Length -gt 120) { $label = $label.Substring(0, 120) }
$group = if ($Alone) { 'alone' } else { 'default' }
$wrapper = @('powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', 'C:\wc\rig\job.ps1')
if ($Alone) { $wrapper += '-Alone' }
$id = & $pueue -c $config add --print-task-id --group $group --priority $Priority --working-directory $checkout --label $label -- @wrapper $Command
if ($LASTEXITCODE -ne 0 -or -not $id) { throw 'pueue add failed' }
$id = "$id".Trim()
[Console]::Error.WriteLine("gpu-job: task $id in group $group at priority $Priority")
& $pueue -c $config wait $id --quiet | Out-Null

$task = (& $pueue -c $config status --json | ConvertFrom-Json).tasks.$id
$log = "C:\wc\pueue\data\task_logs\$id.log"
if (Test-Path $log) { [Console]::Out.Write([IO.File]::ReadAllText($log)) }
$result = $task.status.Done.result
$start = [datetime]$task.status.Done.start
$enqueued = [datetime]$task.status.Done.enqueued_at
$end = [datetime]$task.status.Done.end
[Console]::Error.WriteLine(("gpu-job: task $id queued {0:HH:mm:ss.fff}, started {1:HH:mm:ss.fff}, ended {2:HH:mm:ss.fff}: {3}" -f $enqueued, $start, $end, ($result | ConvertTo-Json -Compress)))
& $pueue -c $config remove $id | Out-Null
if ($result -eq 'Success') { exit 0 }
if ($null -ne $result.Failed) { exit $result.Failed }
exit 124
