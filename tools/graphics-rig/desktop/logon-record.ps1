# Run by the scheduled task wc-failed-logons for each failed logon (Security event 4625) and account
# lockout (4740) as it is logged: appends the event to C:\wc\logs\failed-logons.log, one line, with
# the time, the account, the logon type, the status, the process that asked and where from, and that
# process's command line and parent while it still runs, so the cause of the next lockout leaves
# evidence after the Security log has been overwritten.
param([Parameter(Mandatory)][long]$RecordId)
$ErrorActionPreference = 'Stop'
$log = 'C:\wc\logs\failed-logons.log'
New-Item -ItemType Directory -Force C:\wc\logs | Out-Null
function Append([string]$Line) {
    for ($try = 0; ; $try++) {
        try { [IO.File]::AppendAllLines($log, [string[]]@($Line)); return }
        catch [System.IO.IOException] { if ($try -ge 100) { throw }; Start-Sleep -Milliseconds 50 }
    }
}
try {
    $entry = Get-WinEvent -LogName Security -FilterXPath "*[System[EventRecordID=$RecordId]]" -MaxEvents 1
    $data = @{}
    foreach ($item in ([xml]$entry.ToXml()).Event.EventData.Data) { $data[$item.Name] = $item.'#text' }
    $time = '{0:yyyy-MM-dd HH:mm:ss.fff}' -f $entry.TimeCreated
    if ($entry.Id -eq 4740) {
        Append "$time 4740 account $($data.TargetUserName) locked out, caller computer $($data.TargetDomainName), by $($data.SubjectDomainName)\$($data.SubjectUserName)"
    } else {
        $process = "$($data.ProcessName) (pid $([Convert]::ToInt64("$($data.ProcessId)", 16)))"
        $alive = Get-CimInstance Win32_Process -Filter "ProcessId=$([Convert]::ToInt64("$($data.ProcessId)", 16))" -ErrorAction SilentlyContinue
        $more = if ($alive) {
            $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($alive.ParentProcessId)" -ErrorAction SilentlyContinue
            "; command line: $($alive.CommandLine); parent: $($alive.ParentProcessId) $($parent.ExecutablePath)"
        } else { '; the process had ended' }
        Append ("$time 4625 account $($data.TargetDomainName)\$($data.TargetUserName), logon type $($data.LogonType), " +
            "status $($data.Status) $($data.SubStatus), logon process $($data.LogonProcessName) $($data.AuthenticationPackageName), " +
            "process $process, workstation $($data.WorkstationName), source $($data.IpAddress):$($data.IpPort)$more")
    }
} catch {
    Append ('{0:yyyy-MM-dd HH:mm:ss.fff} event record {1}: could not be read: {2}' -f (Get-Date), $RecordId, $_)
}
