# Real programs' OpenGL renderers on the driver against two real implementations, on the Windows
# desktop as a GPU job (the programs are the rig's builds in C:\wc\programs\src; run its
# programs.ps1 -Mode build first):
#
#   tools/graphics-rig/gpu-job --desktop -- powershell -ExecutionPolicy Bypass -File crates\opengl\cts\programs-windows.ps1
#
# For each program: (1) it runs live on the driver for -Seconds, its error output kept (refusals,
# device losses, the slow paths taken); (2) it runs on the system's OpenGL (the GPU vendor's) under
# apitrace, whose recording is cut to its first -Frames frames; (3) the recording is replayed on the
# system's OpenGL, on llvmpipe and on the driver, every frame saved; (4) compare-frames.py compares
# them. Replaying one recording gives each implementation the same calls with the same times, so
# the frames differ only where the implementations draw differently. Results in
# $Root\programs\results\<program>.
param(
    # Programs to run, comma separated; all when empty.
    [string]$Programs = '',
    [int]$Frames = 150,
    # How long a bgfx example runs, live and while recorded (it does not end by itself).
    [int]$Seconds = 10,
    # The longest a Godot demo may take to draw -Frames frames and end.
    [int]$GodotSeconds = 120,
    # The most one channel of a pixel may differ and the pixel still be the same (the rig's
    # tolerance, graphics-test's pixel comparison).
    [int]$Tolerance = 8,
    # Frames kept beside the one where the driver is furthest from both real implementations.
    [int[]]$KeptFrames = @(10, 60, 120)
)
. (Join-Path $PSScriptRoot 'cts-windows.ps1')

$Driver = 'wcontainers'
$Llvmpipe = 'llvmpipe'
# apitrace (MIT), for recording and replaying only.
$ApitraceVersion = '14.0'
$ApitraceSha256 = '32c702683770ba00b06a12a95fe1afdd7d7353cf519adf2073e6433f78147dc3'
$ApitraceBin = Join-Path $Tools "apitrace-$ApitraceVersion\apitrace-$ApitraceVersion-win64\bin"
# Pillow (MIT-CMU), for comparing frames only.
$PillowVersion = '12.0.0'
$PillowDir = Join-Path $Tools "pillow-$PillowVersion"
# py7zr (LGPL-2.1), only to unpack apitrace's archive, which Windows' tar cannot.
$Py7zrDir = Join-Path $Tools 'py7zr-1.0.0'
$ProgramsSource = 'C:\wc\programs\src'
$Work = Join-Path $Root 'programs'
$Results = Join-Path $Work 'results'

function Install-Tools {
    if (-not (Test-Path (Join-Path $ApitraceBin 'glretrace.exe'))) {
        $archive = Fetch "https://github.com/apitrace/apitrace/releases/download/$ApitraceVersion/apitrace-$ApitraceVersion-win64.7z" "apitrace-$ApitraceVersion-win64.7z"
        if ((Get-FileHash -Algorithm SHA256 $archive).Hash.ToLower() -ne $ApitraceSha256) { throw "programs: $archive is not the pinned apitrace" }
        if (-not (Test-Path $Py7zrDir)) { & $Python -m pip install -q --target $Py7zrDir 'py7zr==1.0.0'; Run 'pip install py7zr' }
        $env:PYTHONPATH = $Py7zrDir
        & $Python -m py7zr x $archive (Join-Path $Tools "apitrace-$ApitraceVersion"); Run 'unpacking apitrace'
        Remove-Item Env:\PYTHONPATH
    }
    if (-not (Test-Path $PillowDir)) { & $Python -m pip install -q --target $PillowDir "pillow==$PillowVersion"; Run 'pip install pillow' }
}

# Each program: its executable, working directory and arguments. bgfx's examples that load no art
# of their own, on bgfx's OpenGL renderer; Godot's two platformer demos on its compatibility
# renderer through native OpenGL (opengl3), every fallback off, at a fixed 60 steps a second.
$bgfxExamples = @('01-cubes', '24-nbody', '26-occlusion', '35-dynamic', '37-gpudrivenrendering', '38-bloom')
$godot = Join-Path $ProgramsSource 'godot\Godot_v4.7.2-stable_win64.exe'
$all = [ordered]@{}
foreach ($e in $bgfxExamples) {
    $all["bgfx-$e"] = @{ exe = Join-Path $ProgramsSource "bgfx\.build\win64_vs2022\bin\example-${e}Release.exe"; cwd = Join-Path $ProgramsSource 'bgfx\examples\runtime'; args = @('--gl') }
}
$godotSettings = @(
    '[rendering]',
    'rendering_device/fallback_to_vulkan=false',
    'rendering_device/fallback_to_d3d12=false',
    'rendering_device/fallback_to_opengl3=false',
    'gl_compatibility/fallback_to_native=false',
    'gl_compatibility/fallback_to_angle=false',
    'gl_compatibility/fallback_to_gles=false'
)
foreach ($project in '3d\platformer', '2d\platformer') {
    $tag = 'godot-' + ($project -replace '^(\w+)\\(\w+)$', '$2-$1') + '-gl'
    $all[$tag] = @{ exe = $godot; cwd = Split-Path $godot; project = $project; limit = $GodotSeconds; args = @('--windowed', '--resolution', '960x540', '--fixed-fps', '60', '--disable-vsync', '--rendering-method', 'gl_compatibility', '--rendering-driver', 'opengl3') }
}

# A copy of a Godot demo of our own, with the settings that turn its fallbacks off (the rig's copy
# is the rig's).
function Get-GodotProject([string]$Project) {
    $copy = Join-Path $Work "godot-demo-projects\$Project"
    if (-not (Test-Path $copy)) {
        & robocopy (Join-Path $ProgramsSource "godot-demo-projects\$Project") $copy /E /NFL /NDL /NJH /NJS /NP | Out-Null
        if ($LASTEXITCODE -ge 8) { throw "programs: could not copy the Godot demo $Project" }
    }
    Set-Content -Encoding ascii (Join-Path $copy 'override.cfg') $godotSettings
    $copy
}

# Runs a program until it ends or for $Limit seconds, its output and error output in $Log; returns
# its exit code, or 'stopped' when it was ended at the limit.
function Invoke-Limited([string]$Exe, [string[]]$Arguments, [string]$Cwd, [string]$Log, [int]$Limit) {
    $quoted = ($Arguments | ForEach-Object { if ($_ -match ' ') { "`"$_`"" } else { $_ } }) -join ' '
    $process = Start-Process -FilePath $Exe -ArgumentList $quoted -WorkingDirectory $Cwd -PassThru -NoNewWindow `
        -RedirectStandardOutput "$Log.out" -RedirectStandardError $Log
    # Without its handle taken while it runs, the process object loses the exit code.
    $null = $process.Handle
    if ($process.WaitForExit($Limit * 1000)) { return $process.ExitCode }
    # The process and every child it started (apitrace's inject.exe starts the program).
    & taskkill /T /F /PID $process.Id 2>&1 | Out-Null
    $process.WaitForExit() | Out-Null
    'stopped'
}

Install-Tools
$path = $env:PATH
$chosen = if ($Programs) { $Programs -split ',' | Where-Object { $_ } } else { $all.Keys }
New-Item -ItemType Directory -Force -Path $Results | Out-Null
$liveFolder = Join-Path $Work 'live'
foreach ($name in $chosen) {
    $program = $all[$name]
    if (-not $program) { throw "programs: no program $name" }
    $out = Join-Path $Results $name
    if (Test-Path $out) { Remove-Item -Recurse -Force $out }
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    $arguments = $program.args
    $limit = if ($program.limit) { $program.limit } else { $Seconds }
    if ($program.project) {
        $arguments = @('--path', (Get-GodotProject $program.project)) + $arguments
    }

    # (1) Live on the driver: a hard link of the program beside Mesa's libraries, which Windows
    # loads before the system's opengl32.dll.
    if (Test-Path $liveFolder) { Remove-Item -Recurse -Force $liveFolder }
    New-Item -ItemType Directory -Force -Path $liveFolder | Out-Null
    foreach ($library in Get-ChildItem (Join-Path $MesaBin '*') -Include '*.dll') { Copy-Item -Force $library.FullName $liveFolder }
    $live = Join-Path $liveFolder (Split-Path -Leaf $program.exe)
    New-Item -ItemType HardLink -Path $live -Value $program.exe | Out-Null
    # Mesa's own frame counter, written to files every half second, shows that the driver drew.
    $hud = Join-Path $out 'hud'
    New-Item -ItemType Directory -Force -Path $hud | Out-Null
    $env:GALLIUM_DRIVER = $Driver
    $env:GALLIUM_HUD = 'fps'
    $env:GALLIUM_HUD_DUMP_DIR = $hud
    $liveArguments = if ($program.project) { $arguments + @('--quit-after', "$Frames") } else { $arguments }
    $code = Invoke-Limited $live $liveArguments $program.cwd (Join-Path $out 'live.log') $limit
    Remove-Item Env:\GALLIUM_DRIVER, Env:\GALLIUM_HUD, Env:\GALLIUM_HUD_DUMP_DIR
    $rates = @(Get-ChildItem $hud -File | Get-Content | ForEach-Object { ($_ -split '\s+')[-1] })
    $problems = @(Select-String -Path (Join-Path $out 'live.log') -Pattern 'refused|not built|lost|panicked' | Select-Object -First 3 | ForEach-Object Line)
    Write-Output "programs: $name live on ${Driver}: $code; frames a second, each half second: $($rates -join ' '); $(if ($problems) { $problems -join ' | ' } else { 'no refusal or loss' })"

    # (2) Recorded on the system's OpenGL, cut to its first frames.
    $whole = Join-Path $out 'whole.trace'
    $trace = Join-Path $out 'program.trace'
    $recordArguments = @('trace', '--api', 'gl', '--output', $whole, $program.exe) + $arguments
    if ($program.project) { $recordArguments += @('--quit-after', "$Frames") }
    $code = Invoke-Limited (Join-Path $ApitraceBin 'apitrace.exe') $recordArguments $program.cwd (Join-Path $out 'record.log') $limit
    if (-not (Test-Path $whole)) { Write-Output "programs: $name recorded nothing ($code)"; continue }
    $code = Invoke-Limited (Join-Path $ApitraceBin 'apitrace.exe') @('trim', '--frames', "0-$($Frames - 1)", '--output', $trace, $whole) $out (Join-Path $out 'trim.log') $GodotSeconds
    if (-not (Test-Path $trace)) { Write-Output "programs: ${name}: the recording could not be cut"; continue }
    Remove-Item $whole

    # (3) Replayed on each implementation, every frame saved as <driver>\<frame>.png.
    foreach ($implementation in $System, $Llvmpipe, $Driver) {
        $shots = Join-Path $out $implementation
        New-Item -ItemType Directory -Force -Path $shots | Out-Null
        if ($implementation -ne $System) {
            # Mesa's opengl32.dll loads libgallium_wgl.dll by name, which glretrace's folder lacks.
            $env:TRACE_LIBGL = Join-Path $MesaBin 'opengl32.dll'
            $env:GALLIUM_DRIVER = $implementation
            $env:PATH = "$MesaBin;$path"
        }
        $replayArguments = @('--call-nos=no', "--snapshot-prefix=$shots\", $trace)
        $code = Invoke-Limited (Join-Path $ApitraceBin 'glretrace.exe') $replayArguments $out (Join-Path $out "replay-$implementation.log") $GodotSeconds
        Remove-Item Env:\TRACE_LIBGL, Env:\GALLIUM_DRIVER -ErrorAction SilentlyContinue
        $env:PATH = $path
        $saved = @(Get-ChildItem $shots -Filter '*.png').Count
        Write-Output "programs: $name replayed on ${implementation}: $code, $saved frames"
    }

    # (4) The comparison.
    $env:PYTHONPATH = $PillowDir
    & $Python (Join-Path $PSScriptRoot 'compare-frames.py') $out $System $Llvmpipe $Driver $Tolerance @KeptFrames
    Remove-Item Env:\PYTHONPATH
}
