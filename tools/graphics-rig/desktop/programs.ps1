# The real Direct3D programs the rig runs: freely licensed samples, demos and games, fetched at
# pinned commits or downloaded at pinned hashes and built in C:\wc\programs (-Mode build, not a GPU
# job), then each run as a GPU job on the hidden desktop (-Mode run). A Direct3D 11 program runs
# through the forwarding d3d11.dll, which counts its Direct3D calls, saves the frames listed in
# -Capture and ends it after the last one. A Direct3D 12 program saves its own frames (the
# forwarding library is Direct3D 11 only) and ends itself after the last. With replacement
# libraries (-Dlls), a replacement d3d11.dll sits behind the forwarding one (WC_D3D11) and the
# others beside the program. Writes <Out>\<program>\census.txt (Direct3D 11) and frame-<n>.png,
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
    # Runs each program three times on real Direct3D and keeps the first run's frames as its
    # references, with the allowances the others give; writes <Out>\references\<program> (the
    # allowances and the frames' SHA-256) for the repository.
    [switch]$Record
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
    # MIT.
    'godot-demo-projects' = @{ url = 'https://github.com/godotengine/godot-demo-projects.git'; commit = 'b761b4cd5718a7118f2c95eb55f181d0a5f004f1'; sparse = @('3d/platformer', '2d/platformer') }
}
# Released builds and data, by SHA-256: Godot (MIT), Chocolate Doom (GPL-2.0, run for testing
# only) and Freedoom (BSD-3-Clause).
$downloads = [ordered]@{
    'godot' = @{ url = 'https://github.com/godotengine/godot/releases/download/4.7.2-stable/Godot_v4.7.2-stable_win64.exe.zip'; sha256 = '731980f9608d61333e5baf54a2ef17210acc7a538446c0cb9969f002aca1e953' }
    'chocolate-doom' = @{ url = 'https://github.com/chocolate-doom/chocolate-doom/releases/download/chocolate-doom-3.1.1/chocolate-doom-3.1.1-win64.zip'; sha256 = '58c34c61ae954493fce5ff01fd553898240a0de25658aa97b43ac9510c49581f' }
    'freedoom' = @{ url = 'https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip'; sha256 = '3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59' }
}
$tutorials = 'directx-sdk-samples\C++\Direct3D11\Tutorials'
$bgfxBin = 'bgfx\.build\win64_vs2022\bin'
$bgfxExamples = @('01-cubes', '06-bump', '09-hdr', '16-shadowmaps', '21-deferred', '24-nbody')
$godot = 'godot\Godot_v4.7.2-stable_win64_console.exe'
$godotProjects = @('3d/platformer', '2d/platformer')
# Each program: its executable and working directory under $root, its arguments ({out} is its
# output directory), its environment, and its API (Direct3D 11 unless said). A Direct3D 12
# program writes its own frames as {out}\movie\frame<number>.png.
$all = [ordered]@{}
foreach ($n in 2, 4, 5, 6, 7) {
    $all["sdk-tutorial0$n"] = @{ exe = "$tutorials\x64\Release\Tutorial0$n.exe"; cwd = "$tutorials\Tutorial0$n"; args = '' }
}
foreach ($e in $bgfxExamples) {
    $all["bgfx-$e"] = @{ exe = "$bgfxBin\example-${e}Release.exe"; cwd = 'bgfx\examples\runtime'; args = '--d3d11' }
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
foreach ($project in $godotProjects) {
    $tag = 'godot-' + ($project -replace '^(\w+)/(\w+)$', '$2-$1')
    $common = "--path $root\src\godot-demo-projects\$($project -replace '/', '\') --windowed --resolution 960x540 --fixed-fps 60 --disable-vsync"
    $all["$tag-d3d11"] = @{ exe = $godot; cwd = 'godot'; args = "$common --rendering-method gl_compatibility --rendering-driver opengl3_angle" }
    $all["$tag-d3d12"] = @{ exe = $godot; cwd = 'godot'; api = 'd3d12'; args = "$common --rendering-method forward_plus --rendering-driver d3d12 --write-movie {out}\movie\frame.png --quit-after {last}" }
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

# Downloads a release into C:\wc\downloads, checks its hash and unpacks it into $root\src\<name>.
function Download([string]$name) {
    $download = $downloads[$name]
    $file = "C:\wc\downloads\$(Split-Path -Leaf $download.url)"
    $dir = "$root\src\$name"
    if (Test-Path "$dir\.sha256") {
        if ((Get-Content "$dir\.sha256") -eq $download.sha256) { return }
        Remove-Item -Recurse -Force $dir
    }
    if (-not (Test-Path $file) -or (Get-FileHash -Algorithm SHA256 $file).Hash -ne $download.sha256) {
        "downloading $name"
        Invoke-WebRequest -UseBasicParsing $download.url -OutFile $file
    }
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash
    if ($hash -ne $download.sha256) { throw "$file has SHA-256 $hash, not $($download.sha256)" }
    Expand-Archive -Force $file $dir
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

# Run: each chosen program in its own directory of copies, with the forwarding library as
# d3d11.dll (Direct3D 11 programs) and any replacements, until its last frame or the time limit.
# Returns whether it presented its last frame and saved every frame, after printing why not.
function Run-Program([string]$name, [string]$dir) {
    $program = $all[$name]
    $d3d12 = $program.api -eq 'd3d12'
    # Through rd with the long path prefix, as programs' caches can go past 260 characters.
    if (Test-Path $dir) { cmd.exe /d /c "rd /s /q `"\\?\$dir`"" }
    $bin = "$dir\bin"
    New-Item -ItemType Directory -Force $bin | Out-Null
    $exe = "$root\src\$($program.exe)"
    # The program's own directory is copied whole: some programs load libraries from beside them.
    Copy-Item "$(Split-Path $exe)\*" $bin -Recurse -Exclude '*.pdb'
    if ($d3d12) { New-Item -ItemType Directory -Force "$dir\movie" | Out-Null } else { Copy-Item $Forward "$bin\d3d11.dll" }
    foreach ($pair in $extra) { Copy-Item $pair[1] "$bin\$($pair[0])" }
    if ($replacement -and $d3d12) {
        Copy-Item $replacement "$bin\d3d11.dll"
    } elseif ($replacement) {
        New-Item -ItemType Directory -Force "$bin\replacement" | Out-Null
        Copy-Item $replacement "$bin\replacement\d3d11.dll"
    }
    $arguments = $program.args.Replace('{out}', $dir).Replace('{last}', "$last")
    # Through gt-launch, which reports where the program loaded each replacement from.
    $names = @($extra | ForEach-Object { $_[0].ToLower() }) + @(if ($replacement -and $d3d12) { 'd3d11.dll' })
    $wanted = if ($names) { $names -join ',' } else { '-' }
    $report = "$dir\loaded.txt"
    $info = New-Object Diagnostics.ProcessStartInfo $Launch, "`"$report`" $wanted `"$bin\$(Split-Path -Leaf $exe)`" $arguments"
    $info.UseShellExecute = $false
    $info.WorkingDirectory = "$root\src\$($program.cwd)"
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    Private-Data $info.EnvironmentVariables $dir
    if ($program.env) { foreach ($key in $program.env.Keys) { $info.EnvironmentVariables[$key] = $program.env[$key] } }
    if (-not $d3d12) {
        $info.EnvironmentVariables['WC_OUT'] = $dir
        $info.EnvironmentVariables['WC_CAPTURE'] = $Capture
        if ($replacement) { $info.EnvironmentVariables['WC_D3D11'] = "$bin\replacement\d3d11.dll" }
    }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $ended = $process.WaitForExit($Limit * 1000)
    if (-not $ended) { & taskkill.exe /T /F /PID $process.Id 2>&1 | Out-Null; $process.WaitForExit() }
    $seconds = [math]::Round($clock.Elapsed.TotalSeconds, 1)
    [IO.File]::WriteAllText("$dir\output.log", $stdout.Result + $stderr.Result)
    if ($d3d12) {
        # The program's own frames, numbered from 0, renamed as the forwarding library names them.
        $movie = @(Get-ChildItem "$dir\movie\frame*.png" -ErrorAction SilentlyContinue | Sort-Object Name)
        $frames = $movie.Count
        foreach ($n in $frameList) { if ($n -le $frames) { Copy-Item $movie[$n - 1].FullName "$dir\frame-$n.png" } }
        $census = 'no census (Direct3D 12)'
        $whole = $ended -and $process.ExitCode -eq 0
    } else {
        $file = "$dir\census.txt"
        $frames = if (Test-Path $file) { [int](((Get-Content $file) -match '^# frames ') -replace '# frames ', '' | Select-Object -First 1) } else { 0 }
        $census = "$(if (Test-Path $file) { @((Get-Content $file) -notmatch '^#').Count } else { 0 }) methods called"
        $whole = $ended
    }
    $saved = @(Get-ChildItem "$dir\frame-*.png" -ErrorAction SilentlyContinue).Count
    $elsewhere = @(if (Test-Path $report) { Get-Content $report | Where-Object { $_ -notmatch ' from not loaded$' } | Where-Object { ($_ -split ' from ', 2)[1] -ine "$bin\$(($_ -split ' from ', 2)[0])" } })
    if ($elsewhere) {
        "${name}: FAIL, did not load the replacement: $($elsewhere -join ', ')"
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

$replacement = $null
$extra = @()
foreach ($pair in ($Dlls.Split(';') | Where-Object { $_ })) {
    $dll, $path = $pair.Split('=', 2)
    if ($dll -eq 'd3d11.dll') { $replacement = (Resolve-Path $path).Path } else { $extra += , @($dll, (Resolve-Path $path).Path) }
}
if ($Record -and $Dlls) { throw 'references are recorded on real Direct3D only' }
$frameList = @($Capture.Split(',') | ForEach-Object { [int]$_ })
$last = $frameList[-1]
$failed = 0
foreach ($name in $chosen) {
    $dir = "$Out\$name"
    $reference = "$References\$name"
    $listing = "tools\graphics-rig\program-references\$name\frames.sha256"
    if ($Record) {
        # Three runs: the first is kept, the others show what the program draws differently each time.
        $runs = @($dir, "$dir-again", "$dir-third")
        $ran = $true
        foreach ($run in $runs) {
            $result = @(Run-Program $name $run)
            $result[0..($result.Count - 2)]
            if (-not $result[-1]) { $ran = $false }
        }
        if (-not $ran) { $failed++; continue }
        if (Test-Path $reference) { Remove-Item -Recurse -Force $reference }
        if (Test-Path "$Out\references\$name") { Remove-Item -Recurse -Force "$Out\references\$name" }
        & $Check frames --record $reference @runs | ForEach-Object { "${name} $_" }
        if ($LASTEXITCODE) { "${name}: FAIL, recording its references failed"; $failed++; continue }
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
    if ($bad) { "${name}: FAIL, frames differ"; $failed++ } else { "${name}: pass, every frame as on real Direct3D" }
}
if ($failed) { exit 1 }
