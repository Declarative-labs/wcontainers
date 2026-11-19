# Runs one rig program for -Seconds with the Direct3D 11 front's libraries (built on the WebGL 2
# back end) beside it and no rig forwarder: the front hands ANGLE's own Direct3D 11 calls to the
# system's library by their caller's module, which the forwarder would hide. Each line the program
# writes is kept with the seconds since its start, so the back end's frame reports
# (wcontainers_gpu_webgl::FRAME_REPORT) give the frames a second. Writes <Out>\output.log.
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Cwd,
    [string]$Arguments = '',
    # Variables for the program, as name=value pairs separated by semicolons.
    [string]$Environment = '',
    [Parameter(Mandatory)][int]$Seconds,
    [Parameter(Mandatory)][string]$Library,
    [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'
$root = 'C:\wc\programs\src'
$bin = "$Out\bin"
if (Test-Path $Out) { cmd.exe /d /c "rd /s /q `"$Out`"" }
New-Item -ItemType Directory -Force $bin, "$Out\appdata", "$Out\localappdata" | Out-Null
Copy-Item "$(Split-Path "$root\$Exe")\*" $bin -Recurse -Exclude '*.pdb'
foreach ($l in 'd3d11.dll', 'dxgi.dll', 'wcontainers_directx.dll') { Copy-Item "$Library\$l" "$bin\$l" }
# An empty NVAPI library, so bgfx does not load NVIDIA's (as the Direct3D 11 track runs it).
New-Item -ItemType File -Force "$bin\nvapi64.dll" | Out-Null
$info = New-Object Diagnostics.ProcessStartInfo "$bin\$(Split-Path -Leaf $Exe)", $Arguments
$info.UseShellExecute = $false
$info.WorkingDirectory = "$root\$Cwd"
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.EnvironmentVariables['APPDATA'] = "$Out\appdata"
$info.EnvironmentVariables['LOCALAPPDATA'] = "$Out\localappdata"
foreach ($pair in $Environment.Split(';', [StringSplitOptions]::RemoveEmptyEntries)) {
    $name, $value = $pair.Split('=', 2)
    $info.EnvironmentVariables[$name] = $value
}
$lines = [Collections.Concurrent.ConcurrentQueue[string]]::new()
$clock = [Diagnostics.Stopwatch]::StartNew()
$process = New-Object Diagnostics.Process
$process.StartInfo = $info
$keep = { if ($null -ne $EventArgs.Data) { $Event.MessageData.Item1.Enqueue("$($Event.MessageData.Item2.Elapsed.TotalSeconds) $($EventArgs.Data)") } }
$data = [Tuple]::Create($lines, $clock)
$o = Register-ObjectEvent $process OutputDataReceived -Action $keep -MessageData $data
$e = Register-ObjectEvent $process ErrorDataReceived -Action $keep -MessageData $data
$process.Start() | Out-Null
$process.BeginOutputReadLine()
$process.BeginErrorReadLine()
$ended = $process.WaitForExit($Seconds * 1000)
if (-not $ended) { & taskkill.exe /T /F /PID $process.Id 2>&1 | Out-Null; $process.WaitForExit() }
Unregister-Event $o.Name
Unregister-Event $e.Name
[IO.File]::WriteAllLines("$Out\output.log", [string[]]$lines.ToArray())
"$(Split-Path -Leaf $Exe): $(if ($ended) { "exited with 0x{0:x8}" -f $process.ExitCode } else { "stopped after $Seconds s" })"
Get-Content "$Out\output.log" | Select-String 'webgl: frames=' | Select-Object -Last 2 | ForEach-Object { $_.Line }
cmd.exe /d /c "rd /s /q `"$bin`""
