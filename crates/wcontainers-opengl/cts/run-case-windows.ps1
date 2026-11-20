# Runs one conformance case alone in glcts on the Windows desktop, with the configuration of the
# list it is in, and prints the program's standard output and error, its exit code, and the case's
# log; a process still running after -Seconds is killed and reported as hung. As a GPU job:
#
#   tools/graphics-rig/gpu-job --desktop -- powershell -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\run-case-windows.ps1 -Driver wcontainers -List gles2-khr-main -Case KHR-GLES2.core.internalformat.texture2d.rgba_unsigned_byte_rgba
#
# With -Debugger the case runs under the LLVM build's lldb, which prints every thread's stack and
# the loaded modules when the program stops on an exception (a crash's place).
# Output in $Root\cts\case\<driver>.
param(
    [Parameter(Mandatory = $true)][string]$Driver,
    [Parameter(Mandatory = $true)][string]$List,
    [Parameter(Mandatory = $true)][string]$Case,
    [int]$Seconds = 60,
    [switch]$Debugger
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

$command = $program
$arguments = $deqpArguments
if ($Debugger) {
    # LLVM's lldb loads the Python it was built with, from a NuGet package as the build's own.
    $lldbPythonVersion = '3.10.11'
    $lldbPython = Join-Path $Tools "python-$lldbPythonVersion"
    if (-not (Test-Path (Join-Path $lldbPython 'tools\python.exe'))) {
        $package = Fetch "https://www.nuget.org/api/v2/package/python/$lldbPythonVersion" "python-$lldbPythonVersion.zip"
        Expand-Archive -Path $package -DestinationPath $lldbPython
    }
    $env:PATH = "$(Join-Path $lldbPython 'tools');$env:PATH"
    $env:PYTHONHOME = Join-Path $lldbPython 'tools'
    # Mesa's PDB files are read by lldb's own reader (Microsoft's needs a registered DLL).
    $env:LLDB_USE_NATIVE_PDB_READER = '1'
    $command = Join-Path $Tools "$LlvmName\bin\lldb.exe"
    # The stopped thread's stack words as symbols too, for frames the unwinder cannot follow.
    $arguments = @('--batch', '-o', 'run', '-k', 'thread backtrace all',
        '-k', 'memory read --format A --size 8 --count 512 $rsp', '-k', 'image list', '-k', 'kill',
        '--', $program) + $deqpArguments
    # Start-Process joins its arguments with spaces, so one holding a space is quoted.
    $arguments = $arguments | ForEach-Object { if ($_ -match ' ') { "`"$_`"" } else { $_ } }
}
$process = Start-Process -FilePath $command -ArgumentList $arguments -WorkingDirectory $output `
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
