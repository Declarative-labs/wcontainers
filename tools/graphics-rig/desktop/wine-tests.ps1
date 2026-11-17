# Builds Wine's Direct3D conformance tests (LGPL, used for testing only, never part of the workspace)
# as 64-bit Windows programs in C:\wc\wine: fetches llvm-mingw and the test sources of a pinned Wine
# commit, and compiles each test file on its own with Wine's test framework in standalone mode.
#   wine-tests.ps1   writes C:\wc\wine\build\<test>_test.exe for d3d11, d3d10core, dxgi and d3d12
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("wine-tests: $_"); exit 1 }
. C:\wc\rig\env.ps1

$wineCommit = '9f8c4ae4d35c7ff4153267aba9618c026f5f5583' # wine-11.22
$mingwRelease = '20261104'
$root = 'C:\wc\wine'
$mingw = "C:\wc\llvm-mingw\llvm-mingw-$mingwRelease-ucrt-x86_64"
New-Item -ItemType Directory -Force $root, "$root\build", C:\wc\llvm-mingw, C:\wc\downloads | Out-Null

if (-not (Test-Path "$mingw\bin\clang.exe")) {
    $zip = "C:\wc\downloads\llvm-mingw-$mingwRelease-ucrt-x86_64.zip"
    if (-not (Test-Path $zip)) {
        curl.exe -sSfL -o $zip "https://github.com/mstorsjo/llvm-mingw/releases/download/$mingwRelease/llvm-mingw-$mingwRelease-ucrt-x86_64.zip"
        if ($LASTEXITCODE) { throw 'llvm-mingw download failed' }
    }
    Expand-Archive $zip C:\wc\llvm-mingw
}

$src = "$root\src"
if (-not (Test-Path "$src\.git")) {
    git clone -q --filter=blob:none --no-checkout --sparse https://gitlab.winehq.org/wine/wine.git $src
    if ($LASTEXITCODE) { throw 'cloning wine failed' }
}
git -C $src sparse-checkout set dlls/d3d11/tests dlls/d3d10core/tests dlls/dxgi/tests dlls/d3d12/tests libs/winecrt0 include/wine include/ddk
if ((git -C $src rev-parse HEAD 2>$null) -ne $wineCommit) {
    git -C $src fetch -q --depth 1 origin $wineCommit
    if ($LASTEXITCODE) { throw "fetching wine $wineCommit failed" }
    git -C $src checkout -q --detach $wineCommit
    if ($LASTEXITCODE) { throw "checking out wine $wineCommit failed" }
}

# The libraries each test links, from its Makefile.in.
$tests = [ordered]@{
    d3d11 = @('d3d11', 'dxgi', 'user32', 'gdi32', 'd3dcompiler')
    d3d10core = @('d3d10', 'dxgi', 'user32')
    dxgi = @('d3d10_1', 'dxgi', 'user32')
    d3d12 = @('d3d12', 'dxgi', 'user32')
}
# Only Wine's own wine/ headers, and the four Windows headers llvm-mingw lacks: Wine's other headers
# (Windows' API, partly generated from .idl files that are not in its sources) would clash with
# llvm-mingw's.
$include = "$root\include"
if (Test-Path $include) { Remove-Item -Recurse -Force $include }
New-Item -ItemType Directory $include | Out-Null
Copy-Item -Recurse "$src\include\wine" "$include\wine"
New-Item -ItemType Directory "$include\ddk" | Out-Null
Copy-Item "$src\include\ddk\d3dkmthk.h" "$include\ddk"
Copy-Item "$src\include\d3dukmdt.h", "$src\include\dxcore.h", "$src\include\dxcore_interface.h" $include
$env:PATH = "$mingw\bin;$env:PATH"
# llvm-mingw has no import library for d3d10_1.dll; the dxgi test calls only D3D10CreateDevice1.
Set-Content "$root\build\d3d10_1.def" "LIBRARY d3d10_1.dll`nEXPORTS`nD3D10CreateDevice1`n"
& llvm-dlltool -m i386:x86-64 -d "$root\build\d3d10_1.def" -l "$root\build\libd3d10_1.a"
if ($LASTEXITCODE) { throw 'making the d3d10_1 import library failed' }
foreach ($test in $tests.Keys) {
    $dir = "$src\dlls\$test\tests"
    # winecrt0's debug.c gives the debug string helpers that Wine's own libraries export.
    $sources = @("$dir\$test.c", "$src\libs\winecrt0\debug.c")
    if (Test-Path "$dir\resource.rc") {
        $object = "$root\build\$test-resource.o"
        & x86_64-w64-mingw32-windres "-I$dir" "$dir\resource.rc" -o $object
        if ($LASTEXITCODE) { throw "windres for $test failed" }
        $sources += $object
    }
    # Macros Wine's own Windows headers define and llvm-mingw's do not.
    $defines = @('-DDECLSPEC_EXPORT=__declspec(dllexport)', '-DARRAY_SIZE(x)=(sizeof(x)/sizeof((x)[0]))', '-DWIDL_C_INLINE_WRAPPERS')
    $libraries = ($tests[$test] + 'ntdll') | ForEach-Object { "-l$_" }
    & x86_64-w64-mingw32-clang -O1 -DSTANDALONE -D__WINESRC__ -D__WINE_PE_BUILD @defines "-I$include" -Wno-everything "-L$root\build" @sources -o "$root\build\${test}_test.exe" @libraries
    if ($LASTEXITCODE) { throw "compiling $test failed" }
    "built $root\build\${test}_test.exe"
}
"wine $wineCommit, llvm-mingw $mingwRelease"
