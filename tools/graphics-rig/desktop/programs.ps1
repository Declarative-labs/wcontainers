# The real Direct3D 11 programs the rig runs: freely licensed samples and demos, fetched at pinned
# commits and built in C:\wc\programs (-Mode build, not a GPU job), then each run as a GPU job on
# the hidden desktop (-Mode run) through the forwarding d3d11.dll, which counts the program's
# Direct3D calls, saves the frames listed in -Capture and ends the program after the last one.
# With replacement libraries (-Dlls), a replacement d3d11.dll sits behind the forwarding one
# (WC_D3D11) and the others beside the program. Writes <Out>\<program>\census.txt and frame-<n>.png.
param(
    [Parameter(Mandatory)][ValidateSet('build', 'run')][string]$Mode,
    # Programs to build or run, comma separated; all when empty.
    [string]$Programs = '',
    [string]$Out,
    # The forwarding library (wcontainers_graphics_test_forward.dll).
    [string]$Forward,
    # Libraries for the programs, as name=path pairs separated by semicolons.
    [string]$Dlls = '',
    [string]$Capture = '10,60,120',
    [int]$Limit = 120
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("programs: $_"); exit 2 }
. C:\wc\rig\env.ps1
$root = 'C:\wc\programs'
$tools = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools'
$msbuild = "$tools\MSBuild\Current\Bin\MSBuild.exe"
$vcvars = "$tools\VC\Auxiliary\Build\vcvars64.bat"

# Sources: where each comes from, its pinned commit and its licence (read at the source).
$sources = [ordered]@{
    'directx-sdk-samples' = @{ url = 'https://github.com/microsoft/DirectX-SDK-Samples.git'; commit = '07e3eaa10e7dd026ec9d95fe326db2d5c4227e1b'; sparse = @('C++/Direct3D11/Tutorials') }
    'bx' = @{ url = 'https://github.com/bkaradzic/bx.git'; commit = '365db1e26b5fdfec2a7497c2f0694c22be08bfbf' }
    'bimg' = @{ url = 'https://github.com/bkaradzic/bimg.git'; commit = 'f091e14e7540c85fd46951342034cfa59ff2693b' }
    'bgfx' = @{ url = 'https://github.com/bkaradzic/bgfx.git'; commit = '3aab3201a949be0d25e20e5fc2afbb2bee80b906' }
}
$tutorials = 'directx-sdk-samples\C++\Direct3D11\Tutorials'
$bgfxBin = 'bgfx\.build\win64_vs2022\bin'
$bgfxExamples = @('01-cubes', '06-bump', '09-hdr', '16-shadowmaps', '21-deferred', '24-nbody')
# Each program: its executable and working directory under $root, and its arguments.
$all = [ordered]@{}
foreach ($n in 2, 4, 5, 6, 7) {
    $all["sdk-tutorial0$n"] = @{ exe = "$tutorials\x64\Release\Tutorial0$n.exe"; cwd = "$tutorials\Tutorial0$n"; args = '' }
}
foreach ($e in $bgfxExamples) {
    $all["bgfx-$e"] = @{ exe = "$bgfxBin\example-${e}Release.exe"; cwd = 'bgfx\examples\runtime'; args = '--d3d11' }
}
$chosen = if ($Programs) { $Programs.Split(',') } else { @($all.Keys) }
foreach ($name in $chosen) { if (-not $all.Contains($name)) { throw "no program $name; the programs are $($all.Keys -join ', ')" } }

function Fetch([string]$name) {
    $source = $sources[$name]
    $dir = "$root\src\$name"
    if (-not (Test-Path "$dir\.git")) {
        git init -q $dir
        git -C $dir remote add origin $source.url
        if ($source.sparse) { git -C $dir sparse-checkout set @($source.sparse) }
    }
    # Through cmd, as Windows PowerShell stops on a native command's error output.
    if ((cmd.exe /d /c "git -C `"$dir`" rev-parse HEAD 2>nul") -ne $source.commit) {
        git -C $dir fetch -q --depth 1 origin $source.commit
        if ($LASTEXITCODE) { throw "fetching $name failed" }
        git -C $dir checkout -q --detach FETCH_HEAD
        if ($LASTEXITCODE) { throw "checking out $name failed" }
    }
}

function Build([string]$what, [string]$command) {
    "building $what"
    cmd.exe /d /c "`"$vcvars`" >nul && $command"
    if ($LASTEXITCODE) { throw "building $what failed" }
}

if ($Mode -eq 'build') {
    New-Item -ItemType Directory -Force "$root\src" | Out-Null
    foreach ($name in $sources.Keys) { Fetch $name }
    $src = "$root\src"
    Build 'the DirectX SDK tutorials' "`"$msbuild`" `"$src\$tutorials\Tutorials_1-7_2022.sln`" /nologo /v:minimal /m /p:Configuration=Release /p:Platform=x64"
    Build 'the bgfx project files' "cd /d `"$src\bgfx`" && ..\bx\tools\bin\windows\genie.exe --with-examples vs2022"
    foreach ($e in $bgfxExamples) {
        Build "bgfx example $e" "`"$msbuild`" `"$src\bgfx\.build\projects\vs2022\example-$e.vcxproj`" /nologo /v:minimal /m /p:Configuration=Release /p:Platform=x64"
    }
    foreach ($name in $all.Keys) {
        if (-not (Test-Path "$src\$($all[$name].exe)")) { throw "$name was not built: no $($all[$name].exe)" }
    }
    "built $($all.Count) programs"
    exit 0
}

# Run: each chosen program in its own directory of copies, with the forwarding library as
# d3d11.dll and any replacements, until its last captured frame or the time limit.
$replacement = $null
$extra = @()
foreach ($pair in ($Dlls.Split(';') | Where-Object { $_ })) {
    $dll, $path = $pair.Split('=', 2)
    if ($dll -eq 'd3d11.dll') { $replacement = (Resolve-Path $path).Path } else { $extra += , @($dll, (Resolve-Path $path).Path) }
}
$failed = 0
foreach ($name in $chosen) {
    $program = $all[$name]
    $dir = "$Out\$name"
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
    $bin = "$dir\bin"
    New-Item -ItemType Directory -Force $bin | Out-Null
    $exe = "$root\src\$($program.exe)"
    Copy-Item $exe $bin
    Copy-Item $Forward "$bin\d3d11.dll"
    foreach ($pair in $extra) { Copy-Item $pair[1] "$bin\$($pair[0])" }
    if ($replacement) {
        New-Item -ItemType Directory -Force "$bin\replacement" | Out-Null
        Copy-Item $replacement "$bin\replacement\d3d11.dll"
    }
    $info = New-Object Diagnostics.ProcessStartInfo "$bin\$(Split-Path -Leaf $exe)", $program.args
    $info.UseShellExecute = $false
    $info.WorkingDirectory = "$root\src\$($program.cwd)"
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.EnvironmentVariables['WC_OUT'] = $dir
    $info.EnvironmentVariables['WC_CAPTURE'] = $Capture
    if ($replacement) { $info.EnvironmentVariables['WC_D3D11'] = "$bin\replacement\d3d11.dll" }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $ended = $process.WaitForExit($Limit * 1000)
    if (-not $ended) { & taskkill.exe /T /F /PID $process.Id 2>&1 | Out-Null; $process.WaitForExit() }
    $seconds = [math]::Round($clock.Elapsed.TotalSeconds, 1)
    [IO.File]::WriteAllText("$dir\output.log", $stdout.Result + $stderr.Result)
    $census = "$dir\census.txt"
    $frames = if (Test-Path $census) { [int](((Get-Content $census) -match '^# frames ') -replace '# frames ', '' | Select-Object -First 1) } else { 0 }
    $last = [int]($Capture.Split(',')[-1])
    $saved = @(Get-ChildItem "$dir\frame-*.png" -ErrorAction SilentlyContinue).Count
    $calls = if (Test-Path $census) { @((Get-Content $census) -notmatch '^#').Count } else { 0 }
    if ($ended -and $frames -ge $last -and $saved -eq $Capture.Split(',').Count) {
        "${name}: pass, $frames frames, $saved saved, $calls methods called, $seconds s"
    } else {
        $why = if (-not $ended) { "still running after $Limit s" } else { ('exited with 0x{0:x8}' -f $process.ExitCode) }
        "${name}: FAIL, $why, $frames frames, $saved of $($Capture.Split(',').Count) frames saved, $seconds s"
        $failed++
    }
}
if ($failed) { exit 1 }
