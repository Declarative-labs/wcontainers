# Builds the pinned conformance suite's glcts (the OpenGL and OpenGL ES lists, on the WGL platform)
# and deqp-runner on the Windows desktop, for testing only. From a checkout on the Mac:
#
#   tools/graphics-rig/remote powershell -ExecutionPolicy Bypass -File crates\opengl\cts\build-windows.ps1
#
# Kept between runs under $Root\cts: src (the suite and the sources its fetch_sources.py pins),
# build, and deqp-runner in $Root\tools\deqp-runner.
. (Join-Path $PSScriptRoot '..\..\nir\mesa\windows-tools.ps1')

$pinFile = Join-Path $PSScriptRoot 'pin.toml'
$Cts = Join-Path $Root 'cts'
$src = Join-Path $Cts 'src'
$build = Join-Path $Cts 'build'
$fetchedMark = Join-Path $src '.git\wcontainers-sources-fetched'

$fresh = Get-PinnedSource (Read-PinField $pinFile 'url') (Read-PinField $pinFile 'tag') (Read-PinField $pinFile 'commit') $src
if ($fresh -or -not (Test-Path $fetchedMark)) {
    if (Test-Path $build) { Remove-Item -Recurse -Force $build }
    & $Python (Join-Path $src 'external\fetch_sources.py'); Run 'fetch_sources.py'
    New-Item -ItemType File -Force -Path $fetchedMark | Out-Null
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
