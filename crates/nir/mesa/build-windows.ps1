# Fetches the build tools, then the pinned Mesa (pin.toml) with the patches of patches/ (applied in
# name order), and builds it with the options of options-windows.ini on the Windows desktop: the WGL
# frontend's opengl32.dll with our driver (crates/opengl) and llvmpipe, by the Visual C++
# compiler. From a checkout on the Mac:
#
#   tools/graphics-rig/remote powershell -ExecutionPolicy Bypass -File crates\nir\mesa\build-windows.ps1
#
# Everything is kept between runs under $Root: tools\ (Python with Meson and Ninja, flex and bison,
# LLVM, the DirectX Shader Compiler), mesa\src, mesa\build and mesa\install. Prints the SHA-256 of
# every download.
. (Join-Path $PSScriptRoot 'windows-tools.ps1')

$FlexBisonVersion = '2.5.25'
$DxcTag = 'v1.9.2609'
$DxcName = 'dxc_2026_09_29'
$DxcUrl = "https://github.com/microsoft/DirectXShaderCompiler/releases/download/$DxcTag/$DxcName.zip"
$FlexBisonUrl = "https://github.com/lexxmark/winflexbison/releases/download/v$FlexBisonVersion/win_flex_bison-$FlexBisonVersion.zip"
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

# The DirectX Shader Compiler: wgpu's Direct3D 12 back end compiles with it where the program's
# folder holds it, as Chrome ships it, and with the older FXC otherwise.
$Dxc = Join-Path $Tools $DxcName
$DxcLibraries = Join-Path $Dxc 'bin\x64'
if (-not (Test-Path (Join-Path $DxcLibraries 'dxcompiler.dll'))) {
    $archive = Fetch $DxcUrl "$DxcName.zip"
    Expand-Archive -Path $archive -DestinationPath $Dxc -Force
}

# LLVM's release for the Visual C++ runtime: its libclang for bindgen and its lldb.
$Llvm = Join-Path $Tools $LlvmName
if (-not (Test-Path (Join-Path $Llvm 'bin\llvm-config.exe'))) {
    $archive = Fetch $LlvmUrl "$LlvmName.tar.xz"
    & $Python -c "import sys, tarfile; tarfile.open(sys.argv[1]).extractall(sys.argv[2], filter='tar')" $archive $Tools
    Run 'unpacking LLVM'
    # The rig's hidden desktop runs conformance cases as a user the unpacked folder does not
    # admit; the local Users group (its well-known SID, as Windows translates the group's name)
    # may read and run it, for lldb.
    & icacls $Llvm /grant '*S-1-5-32-545:(OI)(CI)RX' /T /Q | Out-Null; Run 'icacls LLVM'
}

# llvmpipe's LLVM, built here from the same release's source: the release's own libraries carry
# LLVM's integrated allocator (rpmalloc), which replaces malloc and free in the DLL they are linked
# into while the C runtime's _aligned_realloc still grows blocks in the Windows heap, so Mesa's
# aligned reallocations corrupted the heap. Only the X86 target, the static runtime Mesa uses,
# and the libraries, headers and llvm-config that Meson reads.
$LlvmSourceUrl = "https://github.com/llvm/llvm-project/releases/download/llvmorg-$LlvmVersion/llvm-project-$LlvmVersion.src.tar.xz"
$LlvmBuilt = Join-Path $Tools "llvm-$LlvmVersion-mt"
if (-not (Test-Path (Join-Path $LlvmBuilt 'bin\llvm-config.exe'))) {
    $source = Join-Path $Tools "llvm-project-$LlvmVersion.src"
    if (-not (Test-Path $source)) {
        $archive = Fetch $LlvmSourceUrl "llvm-project-$LlvmVersion.src.tar.xz"
        & $Python -c "import sys, tarfile; tarfile.open(sys.argv[1]).extractall(sys.argv[2], filter='tar')" $archive $Tools
        Run 'unpacking LLVM source'
    }
    $llvmBuild = Join-Path $Tools "llvm-$LlvmVersion-mt-build"
    Enter-VisualCpp
    & cmake -G Ninja -S (Join-Path $source 'llvm') -B $llvmBuild "-DCMAKE_INSTALL_PREFIX=$LlvmBuilt" `
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DLLVM_TARGETS_TO_BUILD=X86 `
        -DLLVM_INCLUDE_TESTS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF -DLLVM_INCLUDE_BENCHMARKS=OFF `
        -DLLVM_ENABLE_ZLIB=OFF -DLLVM_ENABLE_ZSTD=OFF -DLLVM_ENABLE_LIBXML2=OFF -DLLVM_ENABLE_DIA_SDK=OFF
    Run 'cmake LLVM'
    & ninja -C $llvmBuild -j $Jobs install-llvm-headers install-llvm-libraries install-cmake-exports install-llvm-config
    Run 'ninja LLVM'
    Remove-Item -Recurse -Force $llvmBuild, $source
}

$env:PATH = "$FlexBison;$(Join-Path $LlvmBuilt 'bin');$env:PATH"
# bindgen reads Mesa's headers for the driver with the release's libclang.
$env:LIBCLANG_PATH = Join-Path $Llvm 'bin'

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
    # Files an earlier patch added are not the commit's; they go too.
    & git -C $src clean -q -f -d; Run 'git clean'
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
$want = "$commit`n$LlvmBuilt`n$(Get-Content -Raw $options)"
$have = if (Test-Path $configuredFrom) { Get-Content -Raw $configuredFrom } else { $null }
if ($want -ne $have) {
    foreach ($folder in $build, $install) { if (Test-Path $folder) { Remove-Item -Recurse -Force $folder } }
}
$setup = @()
if (Test-Path $build) { $setup = @('--reconfigure') }
Write-Output "build-windows: Mesa $tag ($commit), meson $(& meson --version), LLVM $(& llvm-config --version)"
& meson setup $build $src --vsenv --native-file $options --prefix $install @setup; Run 'meson setup'
Set-Content -NoNewline -Path $configuredFrom -Value $want
# What was built, as the Linux build records it: the commit, the options and the patches (part of
# wcontainers-nir's converter version).
$madeFrom = Join-Path $build 'wcontainers-made-from'
Set-Content -NoNewline -Path $madeFrom -Value "$commit`n$(Get-Content -Raw $options)$wanted"
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

# Gallium's library first, for the headers Mesa generates that the driver's bridge includes.
& meson compile -C $build -j $Jobs gallium; Run 'meson compile gallium'

# Our driver, a static library built against this build's headers and compile flags for the static
# Visual C++ runtime Mesa uses, then linked into libgallium_wgl.dll with the native libraries rustc
# names for it (printed only when rustc runs, so kept between runs).
# The driver links route B through wcontainers-nir, which builds the shaders crate's C++ from its
# submodules (and dxil-spirv's SPIR-V headers).
$repository = Join-Path $Here '..\..\..'
$shaders = 'crates/shaders'
& git -C $repository submodule update -q --init "$shaders/dxbc-spirv" "$shaders/dxil-spirv"; Run 'git submodule update'
& git -C (Join-Path $repository "$shaders/dxil-spirv") submodule update -q --init third_party/spirv-headers
Run 'git submodule update (dxil-spirv)'

$env:WCONTAINERS_MESA = $build
$env:RUSTFLAGS = '-C target-feature=+crt-static'
$driverTarget = Join-Path $Root 'opengl-target'
$nativeLibs = Join-Path $build 'wcontainers-native-libs'
# Windows PowerShell stops on a native command's standard error once it is redirected.
$ErrorActionPreference = 'Continue'
$printed = & cargo rustc --quiet -p wcontainers-opengl --release --lib --target-dir $driverTarget `
    --manifest-path (Join-Path $repository 'Cargo.toml') -- --print native-static-libs 2>&1
$ErrorActionPreference = 'Stop'
if ($LASTEXITCODE -ne 0) { $printed | ForEach-Object { "$_" }; throw "build-windows: cargo rustc wcontainers-opengl failed ($LASTEXITCODE)" }
$found = $printed | ForEach-Object { "$_" } | Select-String -Pattern 'native-static-libs: (.*)$' | Select-Object -First 1
if ($found) { Set-Content -NoNewline -Path $nativeLibs -Value $found.Matches[0].Groups[1].Value }
if (-not (Test-Path $nativeLibs)) { throw 'build-windows: rustc printed no native libraries for the driver' }
$driver = Join-Path $driverTarget 'release\wcontainers_opengl.lib'
$linkArgs = @($driver) + ((Get-Content -Raw $nativeLibs).Trim() -split ' +')
& meson configure $build "-Dwcontainers-link-args=$($linkArgs -join ',')"; Run 'meson configure'
# Meson does not see the library change, so the DLL that links it is always linked again.
Get-ChildItem -Path $build -Recurse -Filter 'libgallium_wgl.dll' | Remove-Item -Force
& meson compile -C $build -j $Jobs; Run 'meson compile'
& meson install -C $build --quiet; Run 'meson install'

foreach ($library in 'opengl32.dll', 'libgallium_wgl.dll') {
    if (-not (Test-Path (Join-Path $install "bin\$library"))) { throw "build-windows: $library was not built" }
}
foreach ($library in 'dxcompiler.dll', 'dxil.dll') {
    Copy-Item -Force (Join-Path $DxcLibraries $library) (Join-Path $install 'bin')
}
Write-Output "build-windows: done, $install"
