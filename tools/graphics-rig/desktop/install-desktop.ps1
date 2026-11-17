# Sets up the route for jobs that present: installs gt-desktop (built by setup-desktop, path given)
# as C:\wc\bin\gt-desktop.exe and registers the scheduled task wc-desktop, which runs it in this
# user's session while the user is logged in at the console (interactive logon, so never without
# that session), any number at once, at the Task Scheduler's default below-normal priority, like
# the queue. Running it again changes nothing that is already in place.
param([Parameter(Mandatory)][string]$Launcher)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("install-desktop: $_"); exit 1 }
New-Item -ItemType Directory -Force C:\wc\desktop | Out-Null
$installed = 'C:\wc\bin\gt-desktop.exe'
if (-not (Test-Path $installed) -or (Get-FileHash $installed).Hash -ne (Get-FileHash $Launcher).Hash) {
    Copy-Item $Launcher $installed -Force
    'installed C:\wc\bin\gt-desktop.exe'
}
$task = 'wc-desktop'
if (-not (Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue)) {
    $action = New-ScheduledTaskAction -Execute $installed -Argument '"$(Arg0)"' -WorkingDirectory 'C:\wc\desktop'
    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries -MultipleInstances Parallel -Hidden
    Register-ScheduledTask -TaskName $task -Action $action -Principal $principal -Settings $settings | Out-Null
    'registered the scheduled task wc-desktop'
}
$session = (Get-Process explorer -ErrorAction SilentlyContinue | Where-Object SessionId -ne 0 | Select-Object -First 1).SessionId
if ($session) { "desktop jobs can run: $env:USERNAME is logged in, session $session" }
else { 'desktop jobs cannot run now: nobody is logged in at the console' }
