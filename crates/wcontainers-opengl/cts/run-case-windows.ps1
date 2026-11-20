# Runs one conformance case alone in glcts on the Windows desktop, with the configuration of the
# list it is in, and prints the program's standard output and error, its exit code, and the case's
# log; a process still running after -Seconds is killed and reported as hung. As a GPU job:
#
#   tools/graphics-rig/gpu-job --desktop -- powershell -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\run-case-windows.ps1 -Driver wcontainers -List gles2-khr-main -Case KHR-GLES2.core.internalformat.texture2d.rgba_unsigned_byte_rgba
#
# Output in $Root\cts\case\<driver>.
param(
    [Parameter(Mandatory = $true)][string]$Driver,
    [Parameter(Mandatory = $true)][string]$List,
    [Parameter(Mandatory = $true)][string]$Case,
    [int]$Seconds = 60
)
. (Join-Path $PSScriptRoot 'cts-windows.ps1')
$program = New-DriverFolder $Driver

$file = Get-Lists | Where-Object { [IO.Path]::GetFileNameWithoutExtension($_) -eq $List } | Select-Object -First 1
if (-not $file) { throw "cts run-case-windows: $List is not in lists.txt" }
$output = Join-Path $Cts "case\$Driver"
if (Test-Path $output) { Remove-Item -Recurse -Force $output }
New-Item -ItemType Directory -Force -Path $output | Out-Null
$log = Join-Path $output 'case.qpa'
$standardOutput = Join-Path $output 'stdout.txt'
$standardError = Join-Path $output 'stderr.txt'
$deqpArguments = @(Get-ListArguments $file) + "--deqp-case=$Case" + "--deqp-log-filename=$log"
Write-Output "cts: $Case on $Driver alone: $deqpArguments"

$process = Start-Process -FilePath $program -ArgumentList $deqpArguments -WorkingDirectory $output `
    -NoNewWindow -PassThru -RedirectStandardOutput $standardOutput -RedirectStandardError $standardError
# The exit code is kept only if the process's handle was opened while it ran.
$null = $process.Handle
$started = Get-Date
$hung = -not $process.WaitForExit($Seconds * [timespan]::TicksPerSecond / [timespan]::TicksPerMillisecond)
if ($hung) {
    $process.Kill()
    $process.WaitForExit()
}
Write-Output '--- standard output'
Get-Content $standardOutput
Write-Output '--- standard error'
Get-Content $standardError
if (Test-Path $log) {
    Write-Output '--- log'
    Get-Content $log
}
if ($hung) {
    Write-Output "cts: $Case on ${Driver}: hung, killed after $Seconds s"
    exit 1
}
Write-Output ('cts: {0} on {1}: exit code 0x{2:x8} after {3:N1} s' -f $Case, $Driver, $process.ExitCode, ((Get-Date) - $started).TotalSeconds)
exit $process.ExitCode
