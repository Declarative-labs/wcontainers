# Exact occlusion counts (GL_SAMPLES_PASSED) on the system's OpenGL, the driver and llvmpipe, each
# against the samples its draws are known to cover (occlusion-count.py). As a GPU job, after Mesa's
# build:
#
#   tools/graphics-rig/gpu-job --desktop -- powershell -ExecutionPolicy Bypass -File crates\opengl\cts\occlusion-windows.ps1
. (Join-Path $PSScriptRoot 'cts-windows.ps1')
& $Python -I (Join-Path $PSScriptRoot 'occlusion-count.py') $MesaBin $System 'wcontainers' 'llvmpipe'
exit $LASTEXITCODE
