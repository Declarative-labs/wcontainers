# One queued GPU job, run by the queue's daemon in C:\wc\src\<Name>: holds that checkout's lock
# shared, so `remote` cannot change it meanwhile, checks that it holds the tree the job was sent for
# (and with -Built that the build directory was built from it since), runs the command (base64 of
# its UTF-8 text, cmd.exe syntax) and exits with its code. The command's standard output goes to the
# job's log; its standard error and this script's own messages go to ErrorFile. With -Alone it
# first stops the default group starting jobs and waits until none of its jobs run, so a timing has
# the GPU to itself, and lets the group go on after.
param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Tree,
    [switch]$Built,
    [switch]$Alone,
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
    # Start-Process gives the command the file itself as its standard error, so processes it leaves
    # behind cannot hold this script up, and its standard output stays the job's log.
    $process = Start-Process cmd.exe -ArgumentList "/d /s /c `"$text`"" -NoNewWindow -PassThru -RedirectStandardError $ErrorFile
    $null = $process.Handle
    $process.WaitForExit()
    $code = $process.ExitCode
} finally {
    if ($Alone) { & $pueue -c $config start --group default | Out-Null }
    $lock.Close()
}
Finish $code
