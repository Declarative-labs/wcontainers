# Registers the scheduled tasks that watch Windows' logs as events come, as this user with full
# rights (needed to read the Security log), whether or not anyone is logged in: wc-failed-logons
# runs logon-record.ps1 for every failed logon (Security event 4625) and account lockout (4740), and
# wc-display-resets runs reset-record.ps1 for every display driver reset or error (System log,
# Display or nvlddmkm, warning or worse). They only read the logs; no audit or security setting is
# changed. A task already in place is left as it is.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("install-watchers: $_"); exit 1 }
New-Item -ItemType Directory -Force C:\wc\logs | Out-Null
$namespace = 'Root/Microsoft/Windows/TaskScheduler'
function Register-Watcher([string]$Task, [string]$Log, [string]$Select, [string]$Script, [string]$Instances) {
    if (Get-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue) { "the scheduled task $Task is in place"; return }
    $trigger = New-CimInstance -CimClass (Get-CimClass -ClassName MSFT_TaskEventTrigger -Namespace $namespace) -ClientOnly
    $trigger.Enabled = $true
    $trigger.Subscription = "<QueryList><Query Id=`"0`" Path=`"$Log`"><Select Path=`"$Log`">$Select</Select></Query></QueryList>"
    $value = New-CimInstance -CimClass (Get-CimClass -ClassName MSFT_TaskNamedValue -Namespace $namespace) -ClientOnly
    $value.Name = 'RecordId'
    $value.Value = 'Event/System/EventRecordID'
    $trigger.ValueQueries = [CimInstance[]]@($value)
    $action = New-ScheduledTaskAction -Execute 'powershell.exe' `
        -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File C:\wc\rig\$Script -RecordId `$(RecordId)"
    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Highest
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 1) -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries -MultipleInstances $Instances -Hidden
    Register-ScheduledTask -TaskName $Task -Action $action -Trigger $trigger -Principal $principal -Settings $settings | Out-Null
    "registered the scheduled task $Task"
}
Register-Watcher 'wc-failed-logons' 'Security' '*[System[(EventID=4625 or EventID=4740)]]' 'logon-record.ps1' 'Parallel'
Register-Watcher 'wc-display-resets' 'System' "*[System[Provider[@Name='Display' or @Name='nvlddmkm'] and (Level=1 or Level=2 or Level=3)]]" 'reset-record.ps1' 'Queue'
