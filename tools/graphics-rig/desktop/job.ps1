# One queued GPU job, run by the queue's daemon: runs the command (base64 of its UTF-8 text, cmd.exe
# syntax) and exits with its code. With -Alone it first stops the default group starting jobs and
# waits until none of its jobs run, so a timing has the GPU to itself, and lets the group go on after.
param([switch]$Alone, [Parameter(Mandatory)][string]$Command)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("job: $_"); exit 125 }
$pueue = 'C:\wc\bin\pueue.exe'
$config = 'C:\wc\pueue\pueue.yml'

if ($Alone) {
    & $pueue -c $config pause --group default --wait | Out-Null
    while ((& $pueue -c $config status --json --group default | ConvertFrom-Json).tasks.PSObject.Properties.Value |
            Where-Object { $_.status.PSObject.Properties.Name -contains 'Running' }) {
        Start-Sleep -Milliseconds 250
    }
    [Console]::Error.WriteLine(("job: alone from {0:HH:mm:ss.fff}" -f (Get-Date)))
}
try {
    $text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Command))
    $info = New-Object System.Diagnostics.ProcessStartInfo 'cmd.exe', "/d /s /c `"$text`""
    $info.UseShellExecute = $false
    $info.WorkingDirectory = (Get-Location).Path
    $process = [System.Diagnostics.Process]::Start($info)
    $process.WaitForExit()
    $code = $process.ExitCode
} finally {
    if ($Alone) { & $pueue -c $config start --group default | Out-Null }
}
exit $code
