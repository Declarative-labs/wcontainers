# Runs on the GPU desktop for `remote`: brings the branch's checkout to the sent commit and runs the
# command in it, with cargo's build directory outside the checkout. A command may reach the GPU, so
# it is watched as a queued job is (C:\wc\rig\hazard.ps1): it is refused while quarantined or while
# the queue is held, and a display driver reset while it runs fails it, holds the queue and
# quarantines it.
param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Commit,
    # A git bundle holding the commit, under refs/wc-remote/, when the public repository lacks it.
    [string]$Bundle,
    # The command, base64 of its UTF-8 text, run by cmd.exe.
    [Parameter(Mandatory)][string]$Command
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("remote: $_"); exit 125 }
. C:\wc\rig\env.ps1
. C:\wc\rig\hazard.ps1
$text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Command))
$quarantined = Find-Quarantine $text
if ($quarantined) { throw "not run: quarantined, as suspected of a display driver reset ($quarantined); see tools/graphics-rig/resets" }
if (Test-Held) { throw "not run: the queue is held after a display driver reset until its cause is recorded (tools/graphics-rig/resets): $((Get-Content $hazardHeld) -join ' / ')" }

# Runs a program with this console's output and error, and fails if it fails.
function Run([string]$File, [string]$Arguments, [string]$Directory = 'C:\wc', [switch]$Passthru) {
    $info = New-Object System.Diagnostics.ProcessStartInfo $File, $Arguments
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $Directory
    $info.EnvironmentVariables['CARGO_TARGET_DIR'] = "C:\wc\target\$Name"
    $process = [System.Diagnostics.Process]::Start($info)
    $process.WaitForExit()
    if ($Passthru) { return $process.ExitCode }
    if ($process.ExitCode -ne 0) { throw "$File $Arguments failed with $($process.ExitCode)" }
}

# One run at a time per branch: the checkout and the build directory are shared.
New-Item -ItemType Directory -Force C:\wc\locks, C:\wc\src, C:\wc\target | Out-Null
$lock = $null
$waited = $false
while (-not $lock) {
    try { $lock = [IO.File]::Open("C:\wc\locks\$Name", 'OpenOrCreate', 'ReadWrite', 'None') }
    catch [System.IO.IOException] {
        if (-not $waited) { [Console]::Error.WriteLine("remote: waiting for another run of $Name"); $waited = $true }
        Start-Sleep -Milliseconds 500
    }
}

$checkout = "C:\wc\src\$Name"
if (-not (Test-Path "$checkout\.git")) {
    Run git "clone -q https://github.com/stack1ng/wcontainers.git $checkout"
}
$known = (Run cmd.exe "/d /c git cat-file -e $Commit^{commit} 2>nul" $checkout -Passthru) -eq 0
if (-not $known) {
    Run git 'fetch -q origin' $checkout
    if ($Bundle) { Run git "fetch -q $Bundle +refs/wc-remote/*:refs/wc-remote/*" $checkout }
}
Run git "checkout -q --force --detach $Commit" $checkout
Run git 'clean -q -fd' $checkout
# Says which tree the build directory was last built from, for `gpu-job --build`; any command may
# change the build directory, so only that build writes it back.
Remove-Item -ErrorAction SilentlyContinue "C:\wc\target\$Name\built-tree"

$suspects = (Get-Quarantine).Count
$started = Get-Date
$code = Run cmd.exe "/d /s /c `"$text`"" $checkout -Passthru
$lock.Close()
$ended = Get-Date
$seen = "{0:yyyy-MM-dd HH:mm:ss} to {1:HH:mm:ss} remote command of ${Name}: {2}" -f $started, $ended, $text
$resets = Watch-Resets $started $ended $seen $text $suspects
if ($resets) {
    [Console]::Error.WriteLine('remote: FAIL, the display driver was reset or logged errors while this command ran (by it or a GPU job):')
    $resets | ForEach-Object { [Console]::Error.WriteLine("remote:   $_") }
    [Console]::Error.WriteLine('remote: this command is now quarantined and the queue is held until the cause is recorded (tools/graphics-rig/resets)')
    if ($code -eq 0) { $code = 1 }
}
if ($Bundle) { Remove-Item $Bundle }
Remove-Item $PSCommandPath
exit $code
