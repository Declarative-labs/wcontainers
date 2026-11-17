# One queued GPU job, run by the queue's daemon in C:\wc\src\<Name>: holds that checkout's lock
# shared, so `remote` cannot change it meanwhile, checks that it holds the tree the job was sent for
# (and with -Built that the build directory was built from it since), runs the command (base64 of
# its UTF-8 text, cmd.exe syntax) and exits with its code. The command's standard output goes to the
# job's log; its standard error and this script's own messages go to ErrorFile. With -Alone it
# first stops the default group starting jobs and waits until none of its jobs run, so a timing has
# the GPU to itself, and lets the group go on after. With -Desktop the command runs, with this
# environment, on the hidden desktop wc-rig of the logged-in user's session (through the scheduled
# task wc-desktop and gt-desktop), where windows and swap chains work; the queue's own session
# refuses swap chains.
param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Tree,
    [switch]$Built,
    [switch]$Alone,
    [switch]$Desktop,
    [Parameter(Mandatory)][string]$ErrorFile,
    [Parameter(Mandatory)][string]$Command
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$pueue = 'C:\wc\bin\pueue.exe'
$config = 'C:\wc\pueue\pueue.yml'
# Messages written before the command runs, added to ErrorFile after it, which the command's
# standard error replaces.
$notes = New-Object System.Collections.Generic.List[string]
function Finish([int]$Code) {
    if ($notes.Count) { [IO.File]::AppendAllLines($ErrorFile, $notes) }
    exit $Code
}
trap { $notes.Add("job: $_"); Finish 125 }

# Runs the command through the task wc-desktop and returns its exit code, its output and error
# copied to this job's.
function RunOnDesktop([string]$Text) {
    $dir = "C:\wc\desktop\$([guid]::NewGuid())"
    New-Item -ItemType Directory -Force $dir | Out-Null
    try {
        [IO.File]::WriteAllText("$dir\command.txt", $Text)
        [IO.File]::WriteAllText("$dir\directory.txt", (Get-Location).Path)
        [IO.File]::WriteAllLines("$dir\environment.txt",
            [string[]](Get-ChildItem env: | ForEach-Object { "$($_.Name)=$($_.Value)" }))
        $service = New-Object -ComObject Schedule.Service
        $service.Connect()
        $null = $service.GetFolder('\').GetTask('wc-desktop').RunEx($dir, 0, 0, '')
        $deadline = (Get-Date).AddSeconds(30)
        while (-not (Test-Path "$dir\running.lock")) {
            if ((Get-Date) -gt $deadline) { throw 'gt-desktop did not start within 30 s: the task wc-desktop runs only while its user is logged in at the console' }
            Start-Sleep -Milliseconds 100
        }
        # gt-desktop holds the lock unshared until it ends.
        while ($true) {
            try { [IO.File]::Open("$dir\running.lock", 'Open', 'ReadWrite', 'None').Close(); break }
            catch [System.IO.IOException] { Start-Sleep -Milliseconds 100 }
        }
        if (Test-Path "$dir\out.log") { [Console]::Out.Write([IO.File]::ReadAllText("$dir\out.log")) }
        if (Test-Path "$dir\err.log") { Copy-Item "$dir\err.log" $ErrorFile -Force }
        if (-not (Test-Path "$dir\exit.txt")) { throw 'gt-desktop ended without an exit code' }
        return [int][IO.File]::ReadAllText("$dir\exit.txt")
    } finally {
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
}

# Shared with other jobs of the branch; `remote` opens it exclusively.
$lock = $null
while (-not $lock) {
    try { $lock = [IO.File]::Open("C:\wc\locks\$Name", 'OpenOrCreate', 'Read', 'Read') }
    catch [System.IO.IOException] { Start-Sleep -Milliseconds 250 }
}
$held = (& git rev-parse 'HEAD^{tree}' | Out-String).Trim()
if ($held -ne $Tree) { throw "C:\wc\src\$Name holds tree $held, not the sent $Tree; send the working tree with remote first" }
if ($Built) {
    $stamp = "C:\wc\target\$Name\built-tree"
    $builtFrom = if (Test-Path $stamp) { [IO.File]::ReadAllText($stamp).Trim() } else { 'nothing' }
    if ($builtFrom -ne $Tree) { throw "C:\wc\target\$Name holds a build of $builtFrom, not of $Tree; another remote ran after this job's build" }
}

try {
    if ($Alone) {
        & $pueue -c $config pause --group default --wait | Out-Null
        while ((& $pueue -c $config status --json --group default | ConvertFrom-Json).tasks.PSObject.Properties.Value |
                Where-Object { $_.status.PSObject.Properties.Name -contains 'Running' }) {
            Start-Sleep -Milliseconds 250
        }
        $notes.Add(("job: alone from {0:HH:mm:ss.fff}" -f (Get-Date)))
    }
    $text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Command))
    if ($Desktop) {
        $code = RunOnDesktop $text
    } else {
        # Start-Process gives the command the file itself as its standard error, so processes it
        # leaves behind cannot hold this script up, and its standard output stays the job's log.
        $process = Start-Process cmd.exe -ArgumentList "/d /s /c `"$text`"" -NoNewWindow -PassThru -RedirectStandardError $ErrorFile
        $null = $process.Handle
        $process.WaitForExit()
        $code = $process.ExitCode
    }
} finally {
    if ($Alone) { & $pueue -c $config start --group default | Out-Null }
    $lock.Close()
}
Finish $code
