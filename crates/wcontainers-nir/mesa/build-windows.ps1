# Fetches the build tools, then the pinned Mesa (pin.toml) with the patches of patches/ (applied in
# name order), and builds it with the options of options-windows.ini on the Windows desktop: the WGL
# frontend's opengl32.dll with llvmpipe, by the Visual C++ compiler. From a checkout on the Mac:
#
#   tools/graphics-rig/remote powershell -ExecutionPolicy Bypass -File crates\wcontainers-nir\mesa\build-windows.ps1
#
# Everything is kept between runs under $Root: tools\ (Python with Meson and Ninja, flex and bison,
# LLVM), mesa\src, mesa\build and mesa\install. Prints the SHA-256 of every download.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Root = 'C:\wc\graphics-nir'
# Half the desktop's threads: the machine is shared with other tracks' builds.
$Jobs = [Math]::Max(1, [Environment]::ProcessorCount / 2)

$PythonVersion = '3.13.1'
$PythonUrl = "https://www.nuget.org/api/v2/package/python/$PythonVersion"
$PythonPackages = @('meson==1.12.1', 'ninja==1.13.0', 'mako==1.3.10', 'PyYAML==6.0.2', 'packaging==25.0')
$FlexBisonVersion = '2.5.25'
$FlexBisonUrl = "https://github.com/lexxmark/winflexbison/releases/download/v$FlexBisonVersion/win_flex_bison-$FlexBisonVersion.zip"
$LlvmVersion = '21.1.8'
$LlvmName = "clang+llvm-$LlvmVersion-x86_64-pc-windows-msvc"
$LlvmUrl = "https://github.com/llvm/llvm-project/releases/download/llvmorg-$LlvmVersion/$LlvmName.tar.xz"

$Here = $PSScriptRoot
$Tools = Join-Path $Root 'tools'
$Mesa = Join-Path $Root 'mesa'
$Downloads = Join-Path $Tools 'downloads'
New-Item -ItemType Directory -Force -Path $Tools, $Mesa, $Downloads | Out-Null

function Fetch([string]$Url, [string]$Name) {
    $file = Join-Path $Downloads $Name
    if (-not (Test-Path $file)) {
        & curl.exe --fail --location --silent --show-error --output "$file.part" $Url
        if ($LASTEXITCODE -ne 0) { throw "build-windows: could not fetch $Url" }
        Move-Item "$file.part" $file
    }
    Write-Host "build-windows: $Name sha256 $((Get-FileHash -Algorithm SHA256 $file).Hash.ToLower())"
    $file
}

function Run([string]$What) {
    if ($LASTEXITCODE -ne 0) { throw "build-windows: $What failed ($LASTEXITCODE)" }
}

# Python from its NuGet package (a whole Python in a zip, nothing installed), with the build's
# Python packages.
$PythonDir = Join-Path $Tools "python-$PythonVersion"
$Python = Join-Path $PythonDir 'tools\python.exe'
if (-not (Test-Path $Python)) {
    $package = Fetch $PythonUrl "python-$PythonVersion.zip"
    Expand-Archive -Path $package -DestinationPath $PythonDir
}
& $Python -m pip install --quiet --disable-pip-version-check @PythonPackages
Run 'pip install'
$Scripts = Join-Path $PythonDir 'tools\Scripts'

# win_flex and win_bison: Mesa's GLSL compiler's lexers and parsers are generated at build time.
$FlexBison = Join-Path $Tools "win_flex_bison-$FlexBisonVersion"
if (-not (Test-Path (Join-Path $FlexBison 'win_bison.exe'))) {
    $archive = Fetch $FlexBisonUrl "win_flex_bison-$FlexBisonVersion.zip"
    Expand-Archive -Path $archive -DestinationPath $FlexBison -Force
}

# LLVM built for the Visual C++ runtime, its libraries and llvm-config: llvmpipe compiles with it.
$Llvm = Join-Path $Tools $LlvmName
if (-not (Test-Path (Join-Path $Llvm 'bin\llvm-config.exe'))) {
    $archive = Fetch $LlvmUrl "$LlvmName.tar.xz"
    & $Python -c "import sys, tarfile; tarfile.open(sys.argv[1]).extractall(sys.argv[2], filter='tar')" $archive $Tools
    Run 'unpacking LLVM'
}

$env:PATH = "$Scripts;$FlexBison;$(Join-Path $Llvm 'bin');$env:PATH"

# The pinned Mesa, with the patches applied again from the pinned commit whenever they change.
$pin = Get-Content (Join-Path $Here 'pin.toml')
function Field([string]$Name) {
    $pattern = "^$Name *= *`"(.*)`"$"
    $line = $pin | Where-Object { $_ -match $pattern } | Select-Object -First 1
    if (-not ($line -match $pattern)) { throw "build-windows: pin.toml lacks $Name" }
    $Matches[1]
}
$url = Field 'url'; $tag = Field 'tag'; $commit = Field 'commit'
$src = Join-Path $Mesa 'src'
$head = if (Test-Path (Join-Path $src '.git')) { & git -C $src rev-parse HEAD 2>$null } else { '' }
if ($head -ne $commit) {
    if (Test-Path $src) { Remove-Item -Recurse -Force $src }
    & git init -q $src; Run 'git init'
    & git -C $src fetch -q --depth 1 $url "refs/tags/$tag"; Run 'git fetch'
    $fetched = & git -C $src rev-parse 'FETCH_HEAD^{commit}'
    if ($fetched -ne $commit) { throw "build-windows: $tag is $fetched upstream, not the pinned $commit" }
    & git -C $src -c advice.detachedHead=false checkout -q $commit; Run 'git checkout'
}
$patches = Get-ChildItem (Join-Path $Here 'patches\*.patch') | Sort-Object Name
$applied = Join-Path $src '.git\wcontainers-patches'
$wanted = ($patches | ForEach-Object { Get-Content -Raw $_.FullName }) -join ''
$had = if (Test-Path $applied) { Get-Content -Raw $applied } else { $null }
if ($wanted -ne $had) {
    & git -C $src checkout -q -f $commit; Run 'git checkout'
    foreach ($patch in $patches) {
        & git -C $src apply --ignore-whitespace $patch.FullName; Run "applying $($patch.Name)"
    }
    Set-Content -NoNewline -Path $applied -Value $wanted
}

# Meson reads a machine file's options only when a build folder is made, so a build made from
# other options or another commit is made again from nothing.
$build = Join-Path $Mesa 'build'
$install = Join-Path $Mesa 'install'
$options = Join-Path $Here 'options-windows.ini'
$configuredFrom = Join-Path $build 'wcontainers-configured-from'
$want = "$commit`n$(Get-Content -Raw $options)"
$have = if (Test-Path $configuredFrom) { Get-Content -Raw $configuredFrom } else { $null }
if ($want -ne $have) {
    foreach ($folder in $build, $install) { if (Test-Path $folder) { Remove-Item -Recurse -Force $folder } }
}
$setup = @()
if (Test-Path $build) { $setup = @('--reconfigure') }
Write-Output "build-windows: Mesa $tag ($commit), meson $(& meson --version), LLVM $(& llvm-config --version)"
& meson setup $build $src --vsenv --native-file $options --prefix $install @setup; Run 'meson setup'
Set-Content -NoNewline -Path $configuredFrom -Value $want
# win_flex names its temporary files the same in every run, so two at once fail: its outputs are
# made first, one at a time (the build steps whose command runs it, read from build.ninja).
$flexOutputs = @()
$outputs = $null
foreach ($line in Get-Content (Join-Path $build 'build.ninja')) {
    if ($line -match '^build (.+?): ') { $outputs = $Matches[1] -split '(?<!\$) ' }
    elseif ($line -match 'win_flex\.exe' -and $outputs) { $flexOutputs += $outputs; $outputs = $null }
}
if ($flexOutputs) {
    & ninja -C $build -j 1 @($flexOutputs | ForEach-Object { $_ -replace '\$(.)', '$1' }); Run 'flex'
}
& meson compile -C $build -j $Jobs; Run 'meson compile'
& meson install -C $build --quiet; Run 'meson install'

foreach ($library in 'opengl32.dll', 'libgallium_wgl.dll') {
    if (-not (Test-Path (Join-Path $install "bin\$library"))) { throw "build-windows: $library was not built" }
}
Write-Output "build-windows: done, $install"
