# Run by the scheduled task wc-display-resets for each display driver reset or error the System log
# gets (Display or nvlddmkm, warning or worse) as it is logged: holds the queue at once, so no GPU job
# starts until the cause is recorded (tools/graphics-rig/resets record), and adds the event and the
# jobs and remote commands running then (by C:\wc\logs\jobs.log) to display-resets.log. Jobs see the
# reset too when they end, and quarantine what they ran.
param([Parameter(Mandatory)][long]$RecordId)
$ErrorActionPreference = 'Stop'
. C:\wc\rig\hazard.ps1
try {
    $entry = Get-WinEvent -LogName System -FilterXPath "*[System[EventRecordID=$RecordId]]" -MaxEvents 1
    $line = Format-DisplayEvent $entry
    # Starts in the last 12 hours with no end after them, by branch, tree and group.
    $since = (Get-Date).AddHours(-12)
    $open = [ordered]@{}
    $history = if (Test-Path "$hazardLogs\jobs.log") { Get-Content "$hazardLogs\jobs.log" -Tail 5000 } else { @() }
    foreach ($row in $history) {
        if ($row -match '^(\S+ \S+) start (\S+ \S+ \S+): ' -and [datetime]$Matches[1] -ge $since) {
            if (-not $open.Contains($Matches[2])) { $open[$Matches[2]] = New-Object System.Collections.Generic.List[string] }
            $open[$Matches[2]].Add($row)
        } elseif ($row -match '^\S+ \S+ end (\S+ \S+ \S+) exit' -and $open.Contains($Matches[1]) -and $open[$Matches[1]].Count) {
            $open[$Matches[1]].RemoveAt(0)
        }
    }
    $running = @($open.Values | ForEach-Object { $_ })
    $lines = @("event $line (seen as it was logged); running then:") + @(if ($running) { $running | ForEach-Object { "  $_" } } else { '  no job or remote command' })
    Add-HazardLines "$hazardLogs\display-resets.log" $lines
    Set-Hold @("display driver event logged: $line")
} catch {
    Add-HazardLines "$hazardLogs\display-resets.log" @(('{0:yyyy-MM-dd HH:mm:ss} System event record {1} could not be read: {2}' -f (Get-Date), $RecordId, $_))
    Set-Hold @("a display driver event was logged (System record $RecordId) but could not be read")
}
