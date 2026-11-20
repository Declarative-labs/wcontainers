# Fetches the build tools, then the pinned Mesa (pin.toml) with the patches of patches/ (applied in
# name order), and builds it with the options of options-windows.ini on the Windows desktop: the WGL
# frontend's opengl32.dll with llvmpipe, by the Visual C++ compiler. From a checkout on the Mac:
#
#   tools/graphics-rig/remote powershell -ExecutionPolicy Bypass -File crates\wcontainers-nir\mesa\build-windows.ps1
#
# Everything is kept between runs under $Root: tools\ (Python with Meson and Ninja, flex and bison,
# LLVM), mesa\src, mesa\build and mesa\install. Prints the SHA-256 of every download.
. (Join-Path $PSScriptRoot 'windows-tools.ps1')

$FlexBisonVersion = '2.5.25'
$FlexBisonUrl = "https://github.com/lexxmark/winflexbison/releases/download/v$FlexBisonVersion/win_flex_bison-$FlexBisonVersion.zip"
$LlvmVersion = '21.1.8'
$LlvmName = "clang+llvm-$LlvmVersion-x86_64-pc-windows-msvc"
$LlvmUrl = "https://github.com/llvm/llvm-project/releases/download/llvmorg-$LlvmVersion/$LlvmName.tar.xz"

$Here = $PSScriptRoot
$Mesa = Join-Path $Root 'mesa'
New-Item -ItemType Directory -Force -Path $Mesa | Out-Null

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

$env:PATH = "$FlexBison;$(Join-Path $Llvm 'bin');$env:PATH"

# The pinned Mesa, with the patches applied again from the pinned commit whenever they change.
$pinFile = Join-Path $Here 'pin.toml'
$tag = Read-PinField $pinFile 'tag'; $commit = Read-PinField $pinFile 'commit'
$src = Join-Path $Mesa 'src'
Get-PinnedSource (Read-PinField $pinFile 'url') $tag $commit $src | Out-Null
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
