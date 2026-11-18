# What keeps the owner's display safe from GPU jobs, dot-sourced by the queue's scripts and the
# test runners. Its state is in C:\wc\logs, shared by every branch:
#   display-resets.log  every reset seen during a job, the jobs it was seen by, and recorded causes
#   held.txt            present while the queue is held after a reset: no GPU job starts until the
#                       cause is recorded (tools/graphics-rig/resets record)
#   quarantine.txt      names never run on the GPU until lifted, one per line: name, time, reason,
#                       tab-separated; a job's name is its command, a Wine test's "wine <file>:<test>"
#   jobs.log            each GPU job's start and end, its branch, group, exit code and command
$hazardLogs = 'C:\wc\logs'
$hazardHeld = "$hazardLogs\held.txt"
$hazardQuarantine = "$hazardLogs\quarantine.txt"
$hazardPueue = 'C:\wc\bin\pueue.exe'
$hazardConfig = 'C:\wc\pueue\pueue.yml'
# The groups whose jobs use the GPU: short jobs, jobs of the kinds seen to reset the driver (one at
# a time), and timings.
$hazardGroups = 'default', 'hazard', 'alone'
New-Item -ItemType Directory -Force $hazardLogs | Out-Null

# Appends lines to a log shared by concurrent jobs, retrying while another holds it.
function Add-HazardLines([string]$Path, [string[]]$Lines) {
    for ($try = 0; ; $try++) {
        try { [IO.File]::AppendAllLines($Path, $Lines); return }
        catch [System.IO.IOException] { if ($try -ge 100) { throw }; Start-Sleep -Milliseconds 50 }
    }
}

# The display driver's resets (a GPU hang the driver recovered from, which blanks the owner's
# screen) and errors logged between two times, as events, and as one line each.
function Get-DisplayEvents([datetime]$From, [datetime]$To) {
    @(Get-WinEvent -ErrorAction SilentlyContinue -FilterHashtable @{
            LogName = 'System'; ProviderName = 'Display', 'nvlddmkm'; StartTime = $From; EndTime = $To; Level = 1, 2, 3 } |
        Sort-Object TimeCreated)
}
function Format-DisplayEvent($Entry) {
    '{0:yyyy-MM-dd HH:mm:ss} {1} {2}: {3}' -f $Entry.TimeCreated, $Entry.ProviderName, $Entry.Id, "$($Entry.Message)".Split("`n")[0].Trim()
}
function Get-DisplayResets([datetime]$From, [datetime]$To) {
    @(Get-DisplayEvents $From $To | ForEach-Object { Format-DisplayEvent $_ })
}

# The quarantined names, each with its line.
function Get-Quarantine {
    $entries = @{}
    if (Test-Path $hazardQuarantine) {
        foreach ($line in [IO.File]::ReadAllLines($hazardQuarantine)) {
            if ($line) { $entries[$line.Split("`t")[0]] = $line }
        }
    }
    $entries
}

function Add-Quarantine([string]$Name, [string]$Reason) {
    if ((Get-Quarantine).ContainsKey($Name)) { return }
    $flat = ($Name -replace '[\t\r\n]', ' ')
    Add-HazardLines $hazardQuarantine @("$flat`t$('{0:yyyy-MM-dd HH:mm:ss}' -f (Get-Date))`t$($Reason -replace '[\t\r\n]', ' ')")
}

# Holds the queue: no job of any GPU group starts until the cause is recorded (resets.ps1); running
# jobs go on (--wait: without it pueue would suspend them).
function Set-Hold([string[]]$Lines) {
    if (-not (Test-Path $hazardHeld)) {
        Add-HazardLines $hazardHeld @("held since $('{0:yyyy-MM-dd HH:mm:ss}' -f (Get-Date)); record the cause with tools/graphics-rig/resets record `"<cause>`"")
    }
    Add-HazardLines $hazardHeld $Lines
    foreach ($group in $hazardGroups) { & $hazardPueue -c $hazardConfig pause --group $group --wait | Out-Null }
}

function Test-Held { Test-Path $hazardHeld }
