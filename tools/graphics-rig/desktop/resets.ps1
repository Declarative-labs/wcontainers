# The display driver's resets and the queue's protection from them (hazard.ps1), for
# tools/graphics-rig/resets: shows the hold, the quarantine and the recorded resets; records the
# cause of a held reset and lets the queue go on; quarantines a name by hand; lifts a quarantine.
# Texts come as base64 of their UTF-8.
param(
    [Parameter(Mandatory)][ValidateSet('show', 'record', 'suspect', 'lift')][string]$Action,
    [string]$Name = '',
    [string]$Text = '',
    [Parameter(Mandatory)][string]$Branch
)
$ErrorActionPreference = 'Stop'
trap { [Console]::Error.WriteLine("resets: $_"); exit 1 }
. $PSScriptRoot\hazard.ps1
function Decode([string]$Value) { [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Value)) }
$now = '{0:yyyy-MM-dd HH:mm:ss}' -f (Get-Date)
switch ($Action) {
    'show' {
        if (Test-Held) { 'the queue is HELD:'; Get-Content $hazardHeld | ForEach-Object { "  $_" } }
        else { 'the queue is not held' }
        $entries = Get-Quarantine
        "$($entries.Count) quarantined (never run on the GPU until lifted):"
        $entries.Values | Sort-Object | ForEach-Object { $name, $since, $reason = $_.Split("`t"); "  $name`n    since ${since}: $reason" }
        'the display resets log, last 30 lines:'
        if (Test-Path "$hazardLogs\display-resets.log") { Get-Content "$hazardLogs\display-resets.log" -Tail 30 | ForEach-Object { "  $_" } }
    }
    'record' {
        $cause = Decode $Text
        if (-not $cause) { throw 'say what caused the reset' }
        if (-not (Test-Held)) { throw 'the queue is not held; nothing to record' }
        $held = Get-Content $hazardHeld
        Add-HazardLines "$hazardLogs\display-resets.log" @("$now cause recorded by ${Branch}: $cause", "  for: $($held -join ' / ')")
        Remove-Item $hazardHeld
        foreach ($group in $hazardGroups) { & $hazardPueue -c $hazardConfig start --group $group | Out-Null }
        "recorded; the queue runs again. Still quarantined: $((Get-Quarantine).Count) (tools/graphics-rig/resets lift <name> lifts one)"
    }
    'suspect' {
        $what = Decode $Name
        if (-not $what) { throw 'name what to quarantine' }
        Add-Quarantine $what "by hand from ${Branch}: $(Decode $Text)"
        "quarantined: $what"
    }
    'lift' {
        $what = Decode $Name
        $entries = Get-Quarantine
        if (-not $entries.ContainsKey($what)) { throw "not quarantined: $what" }
        $kept = @([IO.File]::ReadAllLines($hazardQuarantine) | Where-Object { $_ -and $_.Split("`t")[0] -ne $what })
        [IO.File]::WriteAllLines($hazardQuarantine, [string[]]$kept)
        Add-HazardLines "$hazardLogs\display-resets.log" @("$now quarantine lifted by ${Branch}: $what")
        "lifted: $what"
    }
}
