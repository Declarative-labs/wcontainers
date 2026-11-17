# Builds Wine's Direct3D conformance tests (LGPL, used for testing only, never part of the workspace)
# as 64-bit Windows programs in C:\wc\wine: fetches llvm-mingw and the test sources of a pinned Wine
# commit, and compiles each test file on its own with Wine's test framework in standalone mode. Each
# test its START_TEST runs is made to run only when wanted (wine-select.h), so a run can pick tests by
# name; the copy so changed is the one built, and the names are listed in <test>.tests.
# vkd3d's Direct3D 12 tests (LGPL, the same terms) are built as vkd3d's own "crosstest" build makes
# them, with its test framework, which picks a test by the name in VKD3D_TEST_FILTER (made an exact
# match in the copy of vkd3d_test.h built); its shader runner takes one .shader_test file a run.
#   wine-tests.ps1   writes C:\wc\wine\build\<test>_test.exe and <test>.tests for d3d11, d3d10core,
#                    dxgi and d3d12, and for vkd3d-d3d12, vkd3d-d3d12_invalid_usage,
#                    vkd3d-hlsl_d3d12 and vkd3d-shader_runner
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("wine-tests: $_"); exit 1 }
. C:\wc\rig\env.ps1

$wineCommit = '9f8c4ae4d35c7ff4153267aba9618c026f5f5583' # wine-11.22
$vkd3dCommit = 'fb7f9ff3c1f8d972ee6f1b8c6a92caf6c8ba1e10' # vkd3d-2.1
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
$select = (Resolve-Path tools\graphics-rig\desktop\wine-select.h).Path
foreach ($test in $tests.Keys) {
    $dir = "$src\dlls\$test\tests"
    # Each statement directly in START_TEST's body that names a test_ function becomes
    # `wc_wanted("<name>") ? (void)(<statement>) : (void)0;`.
    $text = [IO.File]::ReadAllText("$dir\$test.c")
    $start = $text.IndexOf("START_TEST($test)")
    if ($start -lt 0) { throw "no START_TEST($test) in $test.c" }
    $body = $text.Substring($start)
    $names = New-Object System.Collections.Generic.List[string]
    $body = [regex]::Replace($body, '(?m)^([ \t]+)([A-Za-z_]\w*\((?:[^;{}])*?\);)', [Text.RegularExpressions.MatchEvaluator] {
            param($m)
            $name = [regex]::Match($m.Groups[2].Value, '\btest_\w+')
            $prefix = $body.Substring(0, $m.Index)
            $depth = ([regex]::Matches($prefix, '\{')).Count - ([regex]::Matches($prefix, '\}')).Count
            if (-not $name.Success -or $depth -ne 1) { return $m.Value }
            if (-not $names.Contains($name.Value)) { $names.Add($name.Value) }
            "$($m.Groups[1].Value)wc_wanted(`"$($name.Value)`") ? (void)($($m.Groups[2].Value.TrimEnd(';'))) : (void)0;"
        })
    [IO.File]::WriteAllText("$root\build\$test.c", $text.Substring(0, $start) + $body)
    [IO.File]::WriteAllLines("$root\build\$test.tests", [string[]]$names)
    # winecrt0's debug.c gives the debug string helpers that Wine's own libraries export.
    $sources = @("$root\build\$test.c", "$src\libs\winecrt0\debug.c")
    if (Test-Path "$dir\resource.rc") {
        $object = "$root\build\$test-resource.o"
        & x86_64-w64-mingw32-windres "-I$dir" "$dir\resource.rc" -o $object
        if ($LASTEXITCODE) { throw "windres for $test failed" }
        $sources += $object
    }
    # Macros Wine's own Windows headers define and llvm-mingw's do not.
    $defines = @('-DDECLSPEC_EXPORT=__declspec(dllexport)', '-DARRAY_SIZE(x)=(sizeof(x)/sizeof((x)[0]))', '-DWIDL_C_INLINE_WRAPPERS')
    $libraries = ($tests[$test] + 'ntdll') | ForEach-Object { "-l$_" }
    & x86_64-w64-mingw32-clang -O1 -DSTANDALONE -D__WINESRC__ -D__WINE_PE_BUILD @defines -include $select "-I$dir" "-I$include" -Wno-everything "-L$root\build" @sources -o "$root\build\${test}_test.exe" @libraries
    if ($LASTEXITCODE) { throw "compiling $test failed" }
    "built $root\build\${test}_test.exe, $($names.Count) tests"
}

# A sparse checkout of a repository at a pinned commit.
function Fetch([string]$url, [string]$dir, [string]$commit, [string[]]$paths) {
    if (-not (Test-Path "$dir\.git")) {
        git clone -q --filter=blob:none --no-checkout --sparse $url $dir
        if ($LASTEXITCODE) { throw "cloning $url failed" }
    }
    git -C $dir sparse-checkout set @paths
    if ((cmd.exe /d /c "git -C `"$dir`" rev-parse HEAD 2>nul") -ne $commit) {
        git -C $dir fetch -q --depth 1 origin $commit
        if ($LASTEXITCODE) { throw "fetching $commit of $url failed" }
        git -C $dir checkout -q --detach $commit
        if ($LASTEXITCODE) { throw "checking out $commit of $url failed" }
    }
}

$vkd3d = "$root\vkd3d"
Fetch https://gitlab.winehq.org/wine/vkd3d.git $vkd3d $vkd3dCommit @('tests', 'include', 'crosslibs')
# The headers vkd3d's build generates from its .idl files, with llvm-mingw's widl.
$generated = "$root\build\vkd3d-include"
New-Item -ItemType Directory -Force "$generated\private" | Out-Null
# Those vkd3d's Makefile.am lists as generated (widl_headers); the others are only imported.
$widlHeaders = [regex]::Matches([IO.File]::ReadAllText("$vkd3d\Makefile.am"), '(?ms)^widl_headers = \\\r?\n(.*?)\r?\n\r?\n').Groups[1].Value
$headers = [regex]::Matches($widlHeaders, '(\S+)\.h') | ForEach-Object { $_.Groups[1].Value }
if (@($headers).Count -lt 10) { throw "found only $(@($headers).Count) headers in vkd3d's widl_headers" }
# widl names each header's guard after the output path given, so it is given the bare name, as
# the shader runner defines some of those guards to keep vkd3d's headers out.
Push-Location $generated
foreach ($header in $headers) {
    $idl = "$vkd3d\$($header.Replace('/', '\')).idl"
    & widl "-I$vkd3d\include" -h -o "$(Split-Path -Leaf $header).h" $idl
    if ($LASTEXITCODE) { throw "widl $idl failed" }
}
Pop-Location
# configure's config.h: nothing in it is needed by the tests built for Windows.
Set-Content "$generated\config.h" ''
$framework = [IO.File]::ReadAllText("$vkd3d\include\private\vkd3d_test.h")
$filter = '!strstr(name, vkd3d_test_state.test_name_filter)'
if (-not $framework.Contains($filter)) { throw "vkd3d_test.h no longer filters tests with $filter" }
[IO.File]::WriteAllText("$generated\private\vkd3d_test.h", $framework.Replace($filter, 'strcmp(name, vkd3d_test_state.test_name_filter)'))
& llvm-dlltool -k -m i386:x86-64 -d "$vkd3d\crosslibs\d3d12.cross64.def" -l "$root\build\libvkd3d-d3d12.a"
if ($LASTEXITCODE) { throw 'making the d3d12 import library failed' }
# The flags of vkd3d's crosstest build.
$flags = @('-O1', '-municode', "-I$generated\private", "-I$vkd3d\include", "-I$vkd3d\include\private", "-I$generated", "-I$vkd3d\tests",
    '-D_WIN32_WINNT=0x0600', '-D__USE_MINGW_ANSI_STDIO=0', '-DVKD3D_CROSSTEST=1', '-Wno-everything')
$libraries = @("$root\build\libvkd3d-d3d12.a", '-ldxgi', '-lgdi32', '-ld3dcompiler_47')
foreach ($test in 'd3d12', 'd3d12_invalid_usage', 'hlsl_d3d12') {
    $text = [IO.File]::ReadAllText("$vkd3d\tests\$test.c")
    $start = $text.IndexOf("START_TEST($test)")
    if ($start -lt 0) { throw "no START_TEST($test) in vkd3d's $test.c" }
    $names = [regex]::Matches($text.Substring($start), 'run_test\((\w+)\)') | ForEach-Object { $_.Groups[1].Value }
    [IO.File]::WriteAllLines("$root\build\vkd3d-$test.tests", [string[]]$names)
    & x86_64-w64-mingw32-clang @flags "$vkd3d\tests\$test.c" -o "$root\build\vkd3d-${test}_test.exe" @libraries
    if ($LASTEXITCODE) { throw "compiling vkd3d's $test failed" }
    "built $root\build\vkd3d-${test}_test.exe, $(@($names).Count) tests"
}
# The shader runner's tests are the .shader_test files vkd3d's Makefile.am lists.
$runner = @('shader_runner', 'shader_runner_d3d9', 'shader_runner_d3d11', 'shader_runner_d3d12') | ForEach-Object { "$vkd3d\tests\$_.c" }
# A test's name is its path under tests with + for / (hlsl+abs.shader_test), as / parts a run's
# chosen tests.
$names = [regex]::Matches([IO.File]::ReadAllText("$vkd3d\Makefile.am"), '(?m)^\ttests/(\S+\.shader_test)') | ForEach-Object { $_.Groups[1].Value.Replace('/', '+') }
[IO.File]::WriteAllLines("$root\build\vkd3d-shader_runner.tests", [string[]]$names)
& x86_64-w64-mingw32-clang @flags @runner -o "$root\build\vkd3d-shader_runner_test.exe" @libraries
if ($LASTEXITCODE) { throw "compiling vkd3d's shader runner failed" }
"built $root\build\vkd3d-shader_runner_test.exe, $(@($names).Count) shader test files"
"wine $wineCommit, vkd3d $vkd3dCommit, llvm-mingw $mingwRelease"
