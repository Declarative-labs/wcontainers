# The real Direct3D programs the rig runs: freely licensed samples, demos and games, fetched at
# pinned commits or downloaded at pinned hashes and built in C:\wc\programs (-Mode build, not a GPU
# job), then each run as a GPU job on the hidden desktop (-Mode run). Each program runs through the
# forwarding library as the library of its API (d3d11.dll or d3d12.dll), which counts its Direct3D
# calls, saves the frames listed in -Capture and ends it after the last one. With replacement
# libraries (-Dlls), a replacement of the program's API library sits behind the forwarding one
# (WC_D3D11, WC_D3D12) and the others beside the program. Writes <Out>\<program>\census.txt and frame-<n>.png,
# and compares the frames with the program's references (see -Record): its frames are kept on this
# machine (they show the programs' own art, under the programs' licences), and the repository
# holds their allowances and SHA-256 (tools\graphics-rig\program-references), so references that
# changed or went missing fail.
param(
    [Parameter(Mandatory)][ValidateSet('build', 'run')][string]$Mode,
    # Programs to run, comma separated; all when empty.
    [string]$Programs = '',
    [string]$Out,
    # The forwarding library (wcontainers_graphics_test_forward.dll).
    [string]$Forward,
    # Libraries for the programs, as name=path pairs separated by semicolons.
    [string]$Dlls = '',
    [string]$Capture = '10,60,120',
    [int]$Limit = 120,
    # gt-run, which compares frames with their references.
    [string]$Check,
    # gt-launch, which runs each program and reports where it loaded the replacements from.
    [string]$Launch,
    # Where each program's reference frames are kept: <References>\<program>.
    [string]$References = 'C:\wc\programs\reference',
    # Run -Run of -Runs of a recording on real Direct3D, each in its own job (a program's few
    # unstable pixels move from job to job, not from run to run within one): runs each program
    # once, and in the last job keeps the first run's frames as its references, with the
    # allowances the others give; writes <Out>\references\<program> (the allowances and the
    # frames' SHA-256) for the repository.
    [switch]$Record,
    [ValidateRange(1, 100)][int]$Run = 1,
    [ValidateRange(2, 100)][int]$Runs = 10
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
    # Without the examples' meshes, textures, fonts, images and texts: bgfx's licence (docs\license.rst)
    # lists several of them with no licence of their own. The examples run draw only bgfx's own
    # shaders and the fonts its overlay embeds (Roboto, Roboto Mono, Kenney, Font Awesome, Codicons).
    'bgfx' = @{ url = 'https://github.com/bkaradzic/bgfx.git'; commit = '3aab3201a949be0d25e20e5fc2afbb2bee80b906'; exclude = @('/examples/assets/', '/examples/runtime/meshes/', '/examples/runtime/textures/', '/examples/runtime/font/', '/examples/runtime/images/', '/examples/runtime/text/') }
    # MIT (one licence for the samples, their kits and media): Microsoft's DirectX Tool Kit samples.
    'xbox-atg-samples' = @{ url = 'https://github.com/microsoft/Xbox-ATG-Samples.git'; commit = 'ff629a6e4876e38fe8761ead116a0e53f634ca67'; sparse = @('PCSamples/IntroGraphics/SimpleBezierPC', 'PCSamples/IntroGraphics/SimpleComputePC', 'PCSamples/IntroGraphics/SimpleInstancingPC', 'PCSamples/IntroGraphics/SimpleLightingPC', 'PCSamples/IntroGraphics/SimpleMSAA_PC', 'Kits/DirectXTK', 'Kits/ATGTK', 'Media/Fonts', 'Media/Textures', 'Media/Meshes/AliasSampleCityBlock') }
    # MIT: Microsoft's Direct3D 12 samples. Their projects take the Agility SDK and DXC from NuGet
    # packages (in $downloads).
    'directx-graphics-samples' = @{ url = 'https://github.com/microsoft/DirectX-Graphics-Samples.git'; commit = '68a2b1e97f71642924e74c7757ca21756ad221b5'; sparse = @('Samples/Desktop/D3D12HelloWorld') }
    # MIT.
    'godot-demo-projects' = @{ url = 'https://github.com/godotengine/godot-demo-projects.git'; commit = 'b761b4cd5718a7118f2c95eb55f181d0a5f004f1'; sparse = @('3d/platformer', '2d/platformer') }
}
$hello = 'directx-graphics-samples\Samples\Desktop\D3D12HelloWorld\src'
# Released builds and data, by SHA-256, each unpacked into $root\src\<name> or its dir: Godot
# (MIT), Chocolate Doom (GPL-2.0, run for testing only), Freedoom (BSD-3-Clause), and the NuGet
# packages the Direct3D 12 samples' projects import from their packages directory, as NuGet's
# packages.config restore would unpack them (the hashes are those nuget.org lists, as SHA-512):
# the Agility SDK (Microsoft's DirectX licence, no acceptance required) and DXC (LLVM's licence).
$downloads = [ordered]@{
    'godot' = @{ url = 'https://github.com/godotengine/godot/releases/download/4.7.2-stable/Godot_v4.7.2-stable_win64.exe.zip'; sha256 = '731980f9608d61333e5baf54a2ef17210acc7a538446c0cb9969f002aca1e953' }
    'chocolate-doom' = @{ url = 'https://github.com/chocolate-doom/chocolate-doom/releases/download/chocolate-doom-3.1.1/chocolate-doom-3.1.1-win64.zip'; sha256 = '58c34c61ae954493fce5ff01fd553898240a0de25658aa97b43ac9510c49581f' }
    'freedoom' = @{ url = 'https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip'; sha256 = '3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59' }
    'agility-sdk' = @{ url = 'https://api.nuget.org/v3-flatcontainer/microsoft.direct3d.d3d12/1.618.3/microsoft.direct3d.d3d12.1.618.3.nupkg'; sha256 = '799981b3569eae3a08468891fd4a0b027907b262a75a5cc3f48d621ee68a21db'; dir = "$hello\packages\Microsoft.Direct3D.D3D12.1.618.3" }
    'dxc' = @{ url = 'https://api.nuget.org/v3-flatcontainer/microsoft.direct3d.dxc/1.8.2505.32/microsoft.direct3d.dxc.1.8.2505.32.nupkg'; sha256 = 'c6e82b70c14552f1dd58e4a79c93eeab1567eeb0a9ee63a51564c410429bce3e'; dir = "$hello\packages\Microsoft.Direct3D.DXC.1.8.2505.32" }
}
$tutorials = 'directx-sdk-samples\C++\Direct3D11\Tutorials'
$bgfxBin = 'bgfx\.build\win64_vs2022\bin'
# Examples that load no meshes, textures or fonts of their own.
$bgfxExamples = @('01-cubes', '24-nbody', '26-occlusion', '35-dynamic', '37-gpudrivenrendering', '38-bloom')
$atgSamples = @('SimpleBezierPC', 'SimpleComputePC', 'SimpleInstancingPC', 'SimpleLightingPC', 'SimpleMSAA_PC')
$atg = 'xbox-atg-samples\PCSamples\IntroGraphics'
# Microsoft's Direct3D 12 samples: a still triangle, and one moved by a constant buffer each frame.
$helloSamples = @('Triangle', 'ConstBuffers')
$godot = 'godot\Godot_v4.7.2-stable_win64_console.exe'
$godotProjects = @('3d/platformer', '2d/platformer')
# Each program: its executable and working directory under $root, its arguments ({out} is its
# output directory), its environment, its API (Direct3D 11 unless said), a part of its frames
# not compared, if any, and a settings file written before it runs ($godotSettings), if any.
$all = [ordered]@{}
foreach ($n in 2, 4, 5, 6, 7) {
    $all["sdk-tutorial0$n"] = @{ exe = "$tutorials\x64\Release\Tutorial0$n.exe"; cwd = "$tutorials\Tutorial0$n"; args = '' }
}
# bgfx's overlay shows the GPU memory the driver reports in use, which differs from run to run and
# between implementations; its line (lower when the example's description takes two lines) is
# left out of the comparison: ignore is x,y,width,height in the frame.
foreach ($e in $bgfxExamples) {
    $all["bgfx-$e"] = @{ exe = "$bgfxBin\example-${e}Release.exe"; cwd = 'bgfx\examples\runtime'; args = '--d3d11'; ignore = '0,236,320,36' }
}
# Two of them on bgfx's Direct3D 12 renderer: the plain cubes and the render-target chain. The
# overlay's GPU time line, above the memory line, is left out too: it shows Direct3D 12 timestamps,
# which the frame clock does not answer.
foreach ($e in '01-cubes', '38-bloom') {
    $all["bgfx-$e-d3d12"] = @{ exe = "$bgfxBin\example-${e}Release.exe"; cwd = 'bgfx\examples\runtime'; args = '--d3d12'; api = 'd3d12'; ignore = '0,208,320,64' }
}
# Microsoft's DirectX Tool Kit samples, run in the root of their repository, where they find their
# media (under Media).
foreach ($sample in $atgSamples) {
    $all["atg-$($sample.ToLower())"] = @{ exe = "$atg\$sample\x64\Release\$sample.exe"; cwd = 'xbox-atg-samples'; args = '' }
}
foreach ($sample in $helloSamples) {
    $all["d3d12-hello-$($sample.ToLower())"] = @{ exe = "$hello\Hello$sample\bin\x64\Release\D3D12Hello$sample.exe"; cwd = "$hello\Hello$sample"; args = ''; api = 'd3d12' }
}
# Freedoom's first demo played as fast as it can be drawn, one game tic a frame, through SDL's
# Direct3D 11 renderer.
$all['chocolate-doom'] = @{
    exe = 'chocolate-doom\chocolate-doom.exe'; cwd = 'chocolate-doom'
    args = "-iwad $root\src\freedoom\freedoom-0.13.0\freedoom1.wad -timedemo demo1 -window -nomouse -nosound -nomusic -config {out}\doom.cfg -extraconfig {out}\extra.cfg"
    env = @{ SDL_RENDER_DRIVER = 'direct3d11' }
}
# Godot's demo games with nobody at the controls, at a fixed 60 steps a second: Direct3D 11
# through Godot's compatibility renderer on ANGLE, and Direct3D 12 through its Forward+ renderer.
# Godot falls back to another renderer (Vulkan, OpenGL) when the one asked for fails; the
# project's override.cfg, which Godot reads after project.godot, turns every fallback off, so a
# failing Direct3D makes the program fail rather than draw through something else.
$godotSettings = @(
    '[rendering]',
    'rendering_device/fallback_to_vulkan=false',
    'rendering_device/fallback_to_d3d12=false',
    'rendering_device/fallback_to_opengl3=false',
    'gl_compatibility/fallback_to_native=false',
    'gl_compatibility/fallback_to_angle=false'
)
foreach ($project in $godotProjects) {
    $tag = 'godot-' + ($project -replace '^(\w+)/(\w+)$', '$2-$1')
    $path = "$root\src\godot-demo-projects\$($project -replace '/', '\')"
    $common = "--path $path --windowed --resolution 960x540 --fixed-fps 60 --disable-vsync"
    $all["$tag-d3d11"] = @{ exe = $godot; cwd = 'godot'; settings = "$path\override.cfg"; args = "$common --rendering-method gl_compatibility --rendering-driver opengl3_angle" }
    $all["$tag-d3d12"] = @{ exe = $godot; cwd = 'godot'; settings = "$path\override.cfg"; api = 'd3d12'; args = "$common --rendering-method forward_plus --rendering-driver d3d12" }
}
$chosen = if ($Programs) { $Programs.Split(',') } else { @($all.Keys) }
foreach ($name in $chosen) { if (-not $all.Contains($name)) { throw "no program $name; the programs are $($all.Keys -join ', ')" } }

function Fetch([string]$name) {
    $source = $sources[$name]
    $dir = "$root\src\$name"
    if (-not (Test-Path "$dir\.git")) {
        git init -q $dir
        git -C $dir remote add origin $source.url
        # Files outside the checkout are never downloaded.
        git -C $dir config remote.origin.promisor true
        git -C $dir config remote.origin.partialclonefilter blob:none
        if ($source.sparse) { git -C $dir sparse-checkout set @($source.sparse) }
        if ($source.exclude) { git -C $dir sparse-checkout set --no-cone '/*' @($source.exclude | ForEach-Object { "!$_" }) }
    }
    # Through cmd, as Windows PowerShell stops on a native command's error output.
    if ((cmd.exe /d /c "git -C `"$dir`" rev-parse HEAD 2>nul") -ne $source.commit) {
        git -C $dir fetch -q --depth 1 --filter=blob:none origin $source.commit
        if ($LASTEXITCODE) { throw "fetching $name failed" }
        git -C $dir checkout -q --detach FETCH_HEAD
        if ($LASTEXITCODE) { throw "checking out $name failed" }
    }
}

# Downloads a release into C:\wc\downloads, checks its hash and unpacks it into $root\src\<name>
# (or its dir).
function Download([string]$name) {
    $download = $downloads[$name]
    $file = "C:\wc\downloads\$(Split-Path -Leaf $download.url)"
    $dir = "$root\src\$(if ($download.dir) { $download.dir } else { $name })"
    if ((Test-Path "$dir\.sha256") -and (Get-Content "$dir\.sha256") -eq $download.sha256) { return }
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
    if (-not (Test-Path $file) -or (Get-FileHash -Algorithm SHA256 $file).Hash -ne $download.sha256) {
        "downloading $name"
        Invoke-WebRequest -UseBasicParsing $download.url -OutFile $file
    }
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash
    if ($hash -ne $download.sha256) { throw "$file has SHA-256 $hash, not $($download.sha256)" }
    # Not Expand-Archive, which takes only files named .zip.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory($file, $dir)
    Set-Content "$dir\.sha256" $download.sha256
}

function Build([string]$what, [string]$command) {
    "building $what"
    cmd.exe /d /c "`"$vcvars`" >nul && $command"
    if ($LASTEXITCODE) { throw "building $what failed" }
}

# Godot keeps its settings, logs and caches under APPDATA and LOCALAPPDATA; these point them into C:\wc.
function Private-Data($environment, [string]$dir) {
    New-Item -ItemType Directory -Force "$dir\appdata", "$dir\localappdata" | Out-Null
    $environment['APPDATA'] = "$dir\appdata"
    $environment['LOCALAPPDATA'] = "$dir\localappdata"
}

if ($Mode -eq 'build') {
    New-Item -ItemType Directory -Force "$root\src", C:\wc\downloads | Out-Null
    foreach ($name in $sources.Keys) { Fetch $name }
    foreach ($name in $downloads.Keys) { Download $name }
    $src = "$root\src"
    Build 'the DirectX SDK tutorials' "`"$msbuild`" `"$src\$tutorials\Tutorials_1-7_2022.sln`" /nologo /v:minimal /m /p:Configuration=Release /p:Platform=x64"
    # Made for Visual Studio 2017: built with this one's toolset and SDK.
    foreach ($sample in $atgSamples) {
        Build "DirectX Tool Kit sample $sample" "`"$msbuild`" `"$src\$atg\$sample\$sample.vcxproj`" /nologo /v:minimal /m /p:Configuration=Release /p:Platform=x64 /p:PlatformToolset=v143 /p:WindowsTargetPlatformVersion=10.0.26100.0"
    }
    foreach ($sample in $helloSamples) {
        Build "Direct3D 12 sample Hello$sample" "`"$msbuild`" `"$src\$hello\Hello$sample\D3D12Hello$sample.vcxproj`" /nologo /v:minimal /m /p:Configuration=Release /p:Platform=x64 /p:PlatformToolset=v143 /p:WindowsTargetPlatformVersion=10.0.26100.0"
    }
    Build 'the bgfx project files' "cd /d `"$src\bgfx`" && ..\bx\tools\bin\windows\genie.exe --with-examples vs2022"
    # bgfx built to draw on the thread that calls bgfx::frame, not on a render thread of its own, so
    # what an example draws in a frame follows the clock it read for that frame (see clock.rs of
    # the forwarding library). MSBuild does not see a change of CL, so a change rebuilds all.
    $defines = '/DBGFX_CONFIG_MULTITHREADED=0'
    $stamp = "$src\bgfx\.build\wc-defines.txt"
    if (-not (Test-Path $stamp) -or (Get-Content $stamp) -ne $defines) {
        if (Test-Path "$src\bgfx\.build\win64_vs2022") { Remove-Item -Recurse -Force "$src\bgfx\.build\win64_vs2022" }
    }
    foreach ($e in $bgfxExamples) {
        Build "bgfx example $e" "set CL=$defines&& `"$msbuild`" `"$src\bgfx\.build\projects\vs2022\example-$e.vcxproj`" /nologo /v:minimal /m /p:Configuration=Release /p:Platform=x64"
    }
    Set-Content $stamp $defines
    # Godot imports a project's assets once, without drawing.
    $env:APPDATA = "$root\godot-data\appdata"
    $env:LOCALAPPDATA = "$root\godot-data\localappdata"
    New-Item -ItemType Directory -Force $env:APPDATA, $env:LOCALAPPDATA | Out-Null
    foreach ($project in $godotProjects) {
        $path = "$src\godot-demo-projects\$($project -replace '/', '\')"
        if (-not (Test-Path "$path\.godot\imported")) {
            "importing $project"
            cmd.exe /d /c "`"$src\$godot`" --headless --path `"$path`" --import 2>&1"
            if ($LASTEXITCODE) { throw "importing $project failed" }
        }
    }
    foreach ($name in $all.Keys) {
        if (-not (Test-Path "$src\$($all[$name].exe)")) { throw "$name was not built: no $($all[$name].exe)" }
    }
    "built $($all.Count) programs"
    exit 0
}

# Run: each chosen program in its own directory of copies, with the forwarding library as the
# library of its API and any replacements, until its last frame or the time limit.
# Returns whether it presented its last frame and saved every frame, after printing why not.
function Run-Program([string]$name, [string]$dir) {
    $program = $all[$name]
    $forwarded = if ($program.api -eq 'd3d12') { 'd3d12.dll' } else { 'd3d11.dll' }
    # Through rd with the long path prefix, as programs' caches can go past 260 characters.
    if (Test-Path $dir) { cmd.exe /d /c "rd /s /q `"\\?\$dir`"" }
    $bin = "$dir\bin"
    New-Item -ItemType Directory -Force $bin | Out-Null
    $exe = "$root\src\$($program.exe)"
    # The program's own directory is copied whole: some programs load libraries from beside them.
    Copy-Item "$(Split-Path $exe)\*" $bin -Recurse -Exclude '*.pdb'
    Copy-Item $Forward "$bin\$forwarded"
    foreach ($pair in $extra) { Copy-Item $pair[1] "$bin\$($pair[0])" }
    # A replacement of the program's API library goes behind the forwarding one; the other API's
    # replacement goes beside the program.
    $beside = @($behind.Keys | Where-Object { $_ -ne $forwarded })
    foreach ($library in $beside) { Copy-Item $behind[$library] "$bin\$library" }
    if ($behind[$forwarded]) {
        New-Item -ItemType Directory -Force "$bin\replacement" | Out-Null
        Copy-Item $behind[$forwarded] "$bin\replacement\$forwarded"
    }
    if ($program.settings) { Set-Content -Encoding ascii $program.settings $godotSettings }
    $arguments = $program.args.Replace('{out}', $dir).Replace('{last}', "$last")
    # Through gt-launch, which reports where the program and the processes it starts loaded each
    # library from: those placed beside it, the library of its API and dxgi.dll.
    $placed = @($extra | ForEach-Object { $_[0].ToLower() }) + $beside + @($forwarded)
    $required = @($forwarded, 'dxgi.dll')
    $names = @($placed + $required | Select-Object -Unique)
    $wanted = $names -join ','
    $report = "$dir\loaded.txt"
    $info = New-Object Diagnostics.ProcessStartInfo $Launch, "`"$report`" $wanted `"$bin\$(Split-Path -Leaf $exe)`" $arguments"
    $info.UseShellExecute = $false
    $info.WorkingDirectory = "$root\src\$($program.cwd)"
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    Private-Data $info.EnvironmentVariables $dir
    if ($program.env) { foreach ($key in $program.env.Keys) { $info.EnvironmentVariables[$key] = $program.env[$key] } }
    $info.EnvironmentVariables['WC_OUT'] = $dir
    $info.EnvironmentVariables['WC_CAPTURE'] = $Capture
    if ($behind[$forwarded]) {
        $info.EnvironmentVariables["WC_$($forwarded.Replace('.dll', '').ToUpper())"] = "$bin\replacement\$forwarded"
    }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $ended = $process.WaitForExit($Limit * 1000)
    if (-not $ended) { & taskkill.exe /T /F /PID $process.Id 2>&1 | Out-Null; $process.WaitForExit() }
    $seconds = [math]::Round($clock.Elapsed.TotalSeconds, 1)
    [IO.File]::WriteAllText("$dir\output.log", $stdout.Result + $stderr.Result)
    cmd.exe /d /c "rd /s /q `"\\?\$bin`""
    $file = "$dir\census.txt"
    $frames = if (Test-Path $file) { [int](((Get-Content $file) -match '^# frames ') -replace '# frames ', '' | Select-Object -First 1) } else { 0 }
    $census = "$(if (Test-Path $file) { @((Get-Content $file) -notmatch '^#').Count } else { 0 }) methods called"
    $whole = $ended
    $saved = @(Get-ChildItem "$dir\frame-*.png" -ErrorAction SilentlyContinue).Count
    # The library of its API and dxgi.dll must have been loaded, and each library placed beside
    # it from there and nowhere else. A program stopped at the time limit has no report (gt-launch
    # is stopped with it).
    $lines = @(if (Test-Path $report) { Get-Content $report })
    $wrong = @(if ($ended -and -not $lines) { "gt-launch wrote no report of the libraries it loaded" })
    # The forwarding library answers timestamp queries from the frame clock but still asks the
    # implementation, and says when it gave an error.
    $wrong += @(($stdout.Result + $stderr.Result) -split "`n" | Where-Object { $_ -match '^d3d11 forward: GetData of a timestamp query failed' } | ForEach-Object { $_.Trim() })
    # A program whose calls are known from its source must make each of them as often as that: its
    # census's count less the calls the graphics libraries made themselves (census.txt says which).
    $expected = "tools\graphics-rig\census\$name.expected"
    if ($ended -and (Test-Path $expected)) {
        $counted = @{}
        $lines = @(if (Test-Path $file) { Get-Content $file })
        foreach ($line in $lines) { if ($line -match '^(\S+ \S+) (\d+)$') { $counted[$Matches[1]] = [long]$Matches[2] } }
        $byLibraries = @{}
        foreach ($line in $lines -match '^# of the counts below, made by the graphics libraries, not the program: ') {
            foreach ($item in ($line -split ': ', 2)[1] -split ', ') { if ($item -match '^(\S+ \S+) (\d+)$') { $byLibraries[$Matches[1]] = [long]$Matches[2] } }
        }
        foreach ($line in @(Get-Content $expected | Where-Object { $_ -and $_ -notmatch '^#' })) {
            $method, $want = $line -split ' (?=\d+$)'
            if (-not $counted.ContainsKey($method)) { $wrong += "census has no $method where the program makes $want"; continue }
            $own = $counted[$method] - [long]$byLibraries[$method]
            if ($own -ne [long]$want) {
                $wrong += "census has $method $own from the program ($($counted[$method]) less $([long]$byLibraries[$method]) from the graphics libraries) where it makes $want"
            }
        }
    }
    foreach ($library in $(if ($lines) { $names })) {
        $from = @($lines | Where-Object { $_ -like "$library from *" } | ForEach-Object { ($_ -split ' from ', 2)[1] })
        if (-not $from -or $from -contains 'not loaded') {
            if ($required -contains $library) { $wrong += "$library not loaded" }
        } elseif ($placed -contains $library) {
            $other = @($from | Where-Object { $_ -ine "$bin\$library" })
            if ($other) { $wrong += "$library loaded from $($other -join ' and '), not from $bin" }
        }
    }
    if ($wrong) {
        "${name}: FAIL, $($wrong -join '; ')"
        return $false
    }
    if ($whole -and $frames -ge $last -and $saved -eq $frameList.Count) {
        "${name}: ran, $frames frames, $saved saved, $census, $seconds s"
        return $true
    }
    $why = if (-not $ended) { "still running after $Limit s" } else { ('exited with 0x{0:x8}' -f $process.ExitCode) }
    "${name}: FAIL, $why, $frames frames, $saved of $($frameList.Count) frames saved, $seconds s"
    return $false
}

# The SHA-256 of each file of a program's references, as "<hash> <file>" lines.
function Reference-Hashes([string]$dir) {
    @(Get-ChildItem $dir -File | Where-Object { $_.Name -ne 'frames.sha256' } | Sort-Object Name | ForEach-Object { "$((Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower()) $($_.Name)" })
}

# Replacements of d3d11.dll and d3d12.dll, which go behind the forwarding library for programs of
# that API, and the other libraries, which go beside every program.
$behind = @{}
$extra = @()
foreach ($pair in ($Dlls.Split(';') | Where-Object { $_ })) {
    $dll, $path = $pair.Split('=', 2)
    if ($dll -in 'd3d11.dll', 'd3d12.dll') { $behind[$dll.ToLower()] = (Resolve-Path $path).Path } else { $extra += , @($dll, (Resolve-Path $path).Path) }
}
if ($Record -and $Dlls) { throw 'references are recorded on real Direct3D only' }
$frameList = @($Capture.Split(',') | ForEach-Object { [int]$_ })
$last = $frameList[-1]
$failed = 0
if ($Run -gt $Runs) { throw "run $Run of $Runs" }
# Only this run's outputs are kept (all of a recording's), without the copies of the programs, so
# the Mac copies no more.
if ((Test-Path $Out) -and $Run -eq 1) { cmd.exe /d /c "rd /s /q `"\\?\$Out`"" }
foreach ($name in $chosen) {
    $dir = "$Out\$name"
    $reference = "$References\$name"
    $listing = "tools\graphics-rig\program-references\$name\frames.sha256"
    if ($Record) {
        # The first run is kept, the others show what the program draws differently each time.
        $dirs = @($dir) + @(2..$Runs | ForEach-Object { "$dir-run$_" })
        $target = $dirs[$Run - 1]
        $result = @(Run-Program $name $target)
        $result[0..($result.Count - 2)]
        if (-not $result[-1]) { $failed++; continue }
        if ($Run -lt $Runs) { continue }
        $missing = @($dirs | Where-Object { -not (Test-Path "$_\frame-$last.png") })
        if ($missing) { "${name}: FAIL, no frames from the earlier runs in $($missing -join ', ')"; $failed++; continue }
        if (Test-Path $reference) { Remove-Item -Recurse -Force $reference }
        if (Test-Path "$Out\references\$name") { Remove-Item -Recurse -Force "$Out\references\$name" }
        $ignore = @(if ($all[$name].ignore) { '--ignore', $all[$name].ignore })
        & $Check frames --record @ignore $reference @dirs | ForEach-Object { "${name} $_" }
        if ($LASTEXITCODE) { "${name}: FAIL, recording its references failed"; $failed++; continue }
        foreach ($d in $dirs[1..($dirs.Count - 1)]) { cmd.exe /d /c "rd /s /q `"\\?\$d`"" }
        New-Item -ItemType Directory -Force "$Out\references\$name" | Out-Null
        Copy-Item "$reference\tolerances.txt" "$Out\references\$name\"
        Set-Content "$Out\references\$name\frames.sha256" (Reference-Hashes $reference)
        "${name}: references recorded in $reference"
        continue
    }
    $result = @(Run-Program $name $dir)
    $result[0..($result.Count - 2)]
    if (-not $result[-1]) { $failed++; continue }
    # The references on this machine must be the ones the repository names.
    if (-not (Test-Path $listing)) { "${name}: FAIL, no references recorded ($listing); run programs run --record"; $failed++; continue }
    $want = @(Get-Content $listing)
    $have = if (Test-Path $reference) { Reference-Hashes $reference } else { @() }
    if (($want -join "`n") -ne ($have -join "`n")) { "${name}: FAIL, the references in $reference are not the ones $listing names; run programs run --record"; $failed++; continue }
    $verdicts = @(& $Check frames $reference $dir)
    $bad = $LASTEXITCODE
    $verdicts | ForEach-Object { "${name} $_" }
    $compared = @($verdicts | Where-Object { $_ -notmatch ': not compared' }).Count
    if ($bad) { "${name}: FAIL, frames differ"; $failed++ }
    elseif ($compared) { "${name}: pass, every compared frame as on real Direct3D" }
    else { "${name}: pass, ran to its last frame; no frame compared, as it draws differently on every run of real Direct3D" }
}
if ($failed) { exit 1 }
