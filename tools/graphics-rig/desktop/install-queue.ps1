# Sets up the GPU job queue on the desktop, once: pueue's daemon as a scheduled task that starts with
# Windows and runs as this user without a window, a default group of short jobs that run at most
# $Parallel at a time, a group 'hazard' for jobs of the kinds that may reset the display driver,
# which run one at a time (beside the short ones), and a group 'alone' for timings that run one at a
# time with the GPU to themselves. Running it again changes nothing that is already in place.
param([int]$Parallel = 4)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("install-queue: $_"); exit 1 }
$pueue = 'C:\wc\bin\pueue.exe'
$config = 'C:\wc\pueue\pueue.yml'
New-Item -ItemType Directory -Force C:\wc\pueue\run | Out-Null
$changed = -not (Test-Path $config) -or ((Get-Content -Raw $config) -ne (Get-Content -Raw C:\wc\rig\pueue.yml))
Copy-Item C:\wc\rig\pueue.yml $config -Force

$task = 'wc-gpu-queue'
if (-not (Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue)) {
    $action = New-ScheduledTaskAction -Execute 'C:\wc\bin\pueued.exe' -Argument "-c $config" -WorkingDirectory 'C:\wc\pueue'
    $trigger = New-ScheduledTaskTrigger -AtStartup
    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 3 `
        -RestartInterval (New-TimeSpan -Minutes 1) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
        -MultipleInstances IgnoreNew
    Register-ScheduledTask -TaskName $task -Action $action -Trigger $trigger -Principal $principal -Settings $settings | Out-Null
    'registered the scheduled task wc-gpu-queue'
}
if ($changed -and (Get-ScheduledTask -TaskName $task).State -eq 'Running') {
    $busy = (& $pueue -c $config status --json | ConvertFrom-Json).tasks.PSObject.Properties.Value |
        Where-Object { $_.status.PSObject.Properties.Name -notcontains 'Done' }
    if ($busy) { throw 'the queue configuration changed but jobs are queued or running; run this again when the queue is idle' }
    Stop-ScheduledTask -TaskName $task
    while (Get-Process pueued -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 250 }
    'stopped the queue daemon to load its changed configuration'
}
$started = $false
if ((Get-ScheduledTask -TaskName $task).State -ne 'Running') {
    Start-ScheduledTask -TaskName $task
    $started = $true
    'started the queue daemon'
}
$deadline = (Get-Date).AddSeconds(30)
while ($true) {
    cmd.exe /d /c "$pueue -c $config status --json >nul 2>nul"
    if ($LASTEXITCODE -eq 0) { break }
    if ((Get-Date) -gt $deadline) { throw 'the queue daemon did not answer within 30 s' }
    Start-Sleep -Milliseconds 500
}
# A daemon that starts with jobs left from before pauses its groups; nobody waits for those jobs now.
if ($started) {
    & $pueue -c $config clean | Out-Null
    if (-not (Test-Path C:\wc\logs\held.txt)) { & $pueue -c $config start --all | Out-Null }
}
$groups = (& $pueue -c $config status --json | ConvertFrom-Json).groups
foreach ($group in 'alone', 'hazard') {
    if (-not $groups.$group) { & $pueue -c $config group add $group --parallel 1 | Out-Null }
    & $pueue -c $config parallel 1 --group $group | Out-Null
}
& $pueue -c $config parallel $Parallel --group default | Out-Null
$groups = (& $pueue -c $config status --json | ConvertFrom-Json).groups
$groups.PSObject.Properties | ForEach-Object { "group $($_.Name): at most $($_.Value.parallel_tasks) at once, $($_.Value.status)" }
