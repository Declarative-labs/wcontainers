# Registers the scheduled task wc-failed-logons, which runs logon-record.ps1 for every failed logon
# (Security event 4625) and account lockout (4740) as Windows logs it, as this user with full
# rights (needed to read the Security log), whether or not anyone is logged in. It reads the log
# only; no audit or security setting is changed. Running it again changes nothing in place.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("install-logons: $_"); exit 1 }
$task = 'wc-failed-logons'
New-Item -ItemType Directory -Force C:\wc\logs | Out-Null
if (Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue) { "the scheduled task $task is in place"; exit 0 }
$namespace = 'Root/Microsoft/Windows/TaskScheduler'
$trigger = New-CimInstance -CimClass (Get-CimClass -ClassName MSFT_TaskEventTrigger -Namespace $namespace) -ClientOnly
$trigger.Enabled = $true
$trigger.Subscription = '<QueryList><Query Id="0" Path="Security"><Select Path="Security">*[System[(EventID=4625 or EventID=4740)]]</Select></Query></QueryList>'
$value = New-CimInstance -CimClass (Get-CimClass -ClassName MSFT_TaskNamedValue -Namespace $namespace) -ClientOnly
$value.Name = 'RecordId'
$value.Value = 'Event/System/EventRecordID'
$trigger.ValueQueries = [CimInstance[]]@($value)
$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File C:\wc\rig\logon-record.ps1 -RecordId $(RecordId)'
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType S4U -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 1) -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries -MultipleInstances Parallel -Hidden
Register-ScheduledTask -TaskName $task -Action $action -Trigger $trigger -Principal $principal -Settings $settings | Out-Null
"registered the scheduled task $task; failed logons go to C:\wc\logs\failed-logons.log"
