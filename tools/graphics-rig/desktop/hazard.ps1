# What keeps the owner's display safe from GPU jobs, dot-sourced by the queue's scripts and the
# test runners. Its state is in C:\wc\logs, shared by every branch:
#   display-resets.log  every reset seen during a job, the jobs it was seen by, and recorded causes
#   held.txt            present while the queue is held after a reset: no GPU job starts until the
#                       cause is recorded (tools/graphics-rig/resets record)
#   quarantine.txt      names never run on the GPU until lifted, one per line: name, time, reason,
#                       tab-separated; a job's name is its command, a Wine test's "wine <file>:<test>"
#   jobs.log            each GPU job's and remote command's start and end, its branch, group,
#                       exit code and command
#   cleared.txt         the rig's own code seen to run without a reset, one key a line (Get-RigKey)
$hazardLogs = 'C:\wc\logs'
$hazardHeld = "$hazardLogs\held.txt"
$hazardQuarantine = "$hazardLogs\quarantine.txt"
$hazardCleared = "$hazardLogs\cleared.txt"
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

# A command's words, paths reduced to their last part, for matching against the quarantine.
function Get-CommandWords([string]$Text) {
    @($Text.ToLower() -split '[\s"&|<>()^]+' | ForEach-Object { ($_ -split '[\\/]')[-1] } | Where-Object { $_ })
}

# The quarantine line of a command, if a quarantined command's words all come in it in that order:
# another output directory or checkout, or anything added, does not get around a quarantine. A
# quarantined Wine test ("wine <file>:<test>") is skipped by its runner, which runs the others; a
# command that runs the test file's program itself (<file>_test.exe) is refused.
function Find-Quarantine([string]$Text) {
    $words = Get-CommandWords $Text
    foreach ($entry in (Get-Quarantine).GetEnumerator()) {
        if ($entry.Key -match '^wine ([^:]+):') {
            $program = "$($Matches[1].ToLower())_test"
            if ($words | Where-Object { $_ -ceq $program -or $_ -ceq "$program.exe" }) { return $entry.Value }
            continue
        }
        $want = Get-CommandWords $entry.Key
        if (-not $want) { continue }
        $found = 0
        foreach ($word in $words) { if ($found -lt $want.Count -and $word -ceq $want[$found]) { $found++ } }
        if ($found -eq $want.Count) { return $entry.Value }
    }
    $null
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

# Lets the GPU groups start jobs again, but not 'default' and 'hazard' while an alone job is queued
# or running: it holds them back from its submission until it ends. -Ending: called by the running
# alone job as it ends, which leaves them held only for the next one queued.
function Resume-Groups([switch]$Ending) {
    $state = & $hazardPueue -c $hazardConfig status --json | ConvertFrom-Json
    $waiting = 'Queued', 'Stashed', 'Locked'
    if (-not $Ending) { $waiting += 'Running' }
    $alone = @($state.tasks.PSObject.Properties.Value | Where-Object { $_.group -eq 'alone' -and
            ($_.status.PSObject.Properties.Name | Where-Object { $_ -in $waiting }) })
    foreach ($group in $hazardGroups) {
        if ($state.groups.$group.status -ne 'Running' -and ($group -eq 'alone' -or $alone.Count -eq 0)) {
            & $hazardPueue -c $hazardConfig start --group $group | Out-Null
            "resumed the group $group"
        }
    }
}

# After a job or a remote command: display driver resets or errors logged while it ran are added to
# display-resets.log, hold the queue and quarantine the command, unless the command quarantined
# something narrower itself (a Wine test, named in the file $Narrowed, which the command finds in
# WC_NARROWED). Every job and command that saw the reset is quarantined so. Returns the events' lines.
function Watch-Resets([datetime]$Started, [datetime]$Ended, [string]$Seen, [string]$Text, [string]$Narrowed) {
    $resets = @(Get-DisplayResets $Started $Ended.AddSeconds(2))
    if ($resets) {
        Add-HazardLines "$hazardLogs\display-resets.log" ([string[]](@($Seen) + $resets))
        Set-Hold @($Seen)
        if (-not ($Narrowed -and [IO.Path]::IsPathRooted($Narrowed) -and (Test-Path $Narrowed))) {
            Add-Quarantine $Text ("display driver reset at {0} during {1}" -f $resets[0].Substring(0, 19), $Seen)
        }
    }
    if ($Narrowed -and [IO.Path]::IsPathRooted($Narrowed)) { Remove-Item -ErrorAction SilentlyContinue $Narrowed }
    $resets
}

# The GPU slot: a job of a kind that may reset the display driver (any job outside the group
# default) holds it alone, so no remote command runs beside it; remote commands share it. A job
# first says it wants the slot by holding the file want open, and remote commands do not start
# while one does, so a stream of them cannot keep it out. Handles close when their process ends.
$hazardSlot = 'C:\wc\locks\gpu-slot'
New-Item -ItemType Directory -Force C:\wc\locks | Out-Null
function Open-Lock([string]$Path, [IO.FileAccess]$Access, [IO.FileShare]$Share) {
    try { [IO.File]::Open($Path, 'OpenOrCreate', $Access, $Share) } catch [System.IO.IOException] { $null }
}
# For a job: returns the slot held alone, waiting for the remote commands that hold it to end.
function Enter-GpuSlot {
    $want = $null
    while (-not $want) { $want = Open-Lock "$hazardSlot.want" 'ReadWrite' 'ReadWrite'; if (-not $want) { Start-Sleep -Milliseconds 100 } }
    $slot = $null
    while (-not $slot) { $slot = Open-Lock $hazardSlot 'ReadWrite' 'None'; if (-not $slot) { Start-Sleep -Milliseconds 250 } }
    $want.Close()
    $slot
}
# For a remote command: returns the slot held shared, once no job wants or holds it; $Waiting is
# called once if it has to wait.
function Enter-GpuSlotShared([scriptblock]$Waiting) {
    $said = $false
    while ($true) {
        $free = Open-Lock "$hazardSlot.want" 'ReadWrite' 'None'
        if ($free) {
            $free.Close()
            $slot = Open-Lock $hazardSlot 'Read' 'Read'
            if ($slot) { return $slot }
        }
        if (-not $said) { & $Waiting; $said = $true }
        Start-Sleep -Milliseconds 500
    }
}

# The rig's own runs on Microsoft's Direct3D, wgpu, Chrome or WARP with no replacement library: the
# programs and scenes, the loop, Wine's tests on WARP and the queue check's sleeps. A command that
# chains or redirects (& | < > ^) is none of them.
$hazardRigRuns = @(
    '^powershell -NoProfile -ExecutionPolicy Bypass -File tools\\graphics-rig\\desktop\\programs\.ps1 -Mode run (?!.*-Dlls)',
    '^%CARGO_TARGET_DIR%\\debug\\gt-run (check|record) (?!.*--dll)',
    '^tools\\graphics-rig\\loop\.cmd ',
    '^powershell -NoProfile -ExecutionPolicy Bypass -File tools\\graphics-rig\\desktop\\wine-run\.ps1 (?=.* -Warp\b)(?!.*-Dlls)',
    '^powershell -NoProfile -ExecutionPolicy Bypass -File tools\\graphics-rig\\desktop\\sleep\.ps1 '
)
function Test-RigRun([string]$Text) {
    $Text -cnotmatch '[&|<>^]' -and @($hazardRigRuns | Where-Object { $Text -cmatch $_ }).Count -gt 0
}

# The rig's code in a tree: the trees of its crates and scripts, comma-separated (a job's arguments
# cannot hold spaces), empty where one is missing.
function Get-RigKey([string]$Checkout, [string]$Tree) {
    $ids = foreach ($path in 'crates/wcontainers-graphics-test', 'crates/wcontainers-graphics-test-forward', 'tools/graphics-rig') {
        (& git -C $Checkout rev-parse -q --verify "${Tree}:$path" | Out-String).Trim()
    }
    $ids -join ','
}

# Whether a job may run beside others: it is one of the rig's own runs (Test-RigRun) and the rig's
# code in its tree has already run a job without a display driver reset. Any other job runs one at
# a time: Wine's and vkd3d's tests on the GPU (they reset the driver six times), replacement
# libraries, other tracks' code, and the rig's runs after any change to the rig (an edited scene
# reset it once), until they have run once.
function Test-Cleared([string]$Text, [string]$Key) {
    (Test-RigRun $Text) -and $Key -notmatch '^,|,$|,,' -and (Test-Path $hazardCleared) -and
    ([IO.File]::ReadAllLines($hazardCleared) -contains $Key)
}

function Add-Cleared([string]$Key) {
    if (-not ((Test-Path $hazardCleared) -and ([IO.File]::ReadAllLines($hazardCleared) -contains $Key))) {
        Add-HazardLines $hazardCleared @($Key)
    }
}

# A remote command (remote.ps1, run from C:\wc\incoming) holds the GPU slot shared from here, before
# its branch's lock, until it ends.
if ($MyInvocation.PSCommandPath -like 'C:\wc\incoming\*') {
    $hazardRemoteSlot = Enter-GpuSlotShared { [Console]::Error.WriteLine('remote: waiting for a GPU job of a kind that may reset the display driver to end (no remote command runs beside one)') }
}
