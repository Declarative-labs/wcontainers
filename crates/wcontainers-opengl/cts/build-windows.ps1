# Builds the pinned conformance suite's glcts (the OpenGL and OpenGL ES lists, on the WGL platform)
# and deqp-runner on the Windows desktop, for testing only. From a checkout on the Mac:
#
#   tools/graphics-rig/remote powershell -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\build-windows.ps1
#
# Kept between runs under $Root\cts: src (the suite and the sources its fetch_sources.py pins),
# build, and deqp-runner in $Root\tools\deqp-runner.
. (Join-Path $PSScriptRoot '..\..\wcontainers-nir\mesa\windows-tools.ps1')

$pinFile = Join-Path $PSScriptRoot 'pin.toml'
$Cts = Join-Path $Root 'cts'
$src = Join-Path $Cts 'src'
$build = Join-Path $Cts 'build'
$fetchedMark = Join-Path $src '.git\wcontainers-sources-fetched'

$commit = Read-PinField $pinFile 'commit'
$fresh = Get-PinnedSource (Read-PinField $pinFile 'url') (Read-PinField $pinFile 'tag') $commit $src
if ($fresh -or -not (Test-Path $fetchedMark)) {
    if (Test-Path $build) { Remove-Item -Recurse -Force $build }
    & $Python (Join-Path $src 'external\fetch_sources.py'); Run 'fetch_sources.py'
    New-Item -ItemType File -Force -Path $fetchedMark | Out-Null
}

# The patches of patches/ (testing only), applied again from the pinned commit whenever they change;
# the patches change tracked files only, so the fetched sources stay.
$patches = @(Get-ChildItem (Join-Path $PSScriptRoot 'patches\*.patch') | Sort-Object Name)
$applied = Join-Path $src '.git\wcontainers-patches'
$wanted = ($patches | ForEach-Object { Get-Content -Raw $_.FullName }) -join ''
$had = if (Test-Path $applied) { Get-Content -Raw $applied } else { '' }
if ($wanted -ne $had) {
    & git -C $src checkout -q -f $commit; Run 'git checkout'
    foreach ($patch in $patches) {
        & git -C $src apply --ignore-whitespace $patch.FullName; Run "applying $($patch.Name)"
    }
    Set-Content -NoNewline -Path $applied -Value $wanted
}

Enter-VisualCpp
& cmake -S $src -B $build -G Ninja -DCMAKE_BUILD_TYPE=Release -DDEQP_TARGET=default; Run 'cmake'
& ninja -C $build -j $Jobs glcts; Run 'ninja glcts'

$runner = Join-Path $Tools 'deqp-runner'
& cargo install --locked --quiet --root $runner --target-dir (Join-Path $Cts 'runner-target') `
    --git (Read-PinField $pinFile 'runner_url') --rev (Read-PinField $pinFile 'runner_commit') deqp-runner
Run 'cargo install deqp-runner'
Remove-Item -Recurse -Force (Join-Path $Cts 'runner-target')

Write-Output "cts build-windows: done, $build"
