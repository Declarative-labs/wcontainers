@echo off
rem One case of the own program, run with real Direct3D 12 and through our libraries: check_case <name>.
set E=%CARGO_TARGET_DIR%\debug\examples
set C=C:\wc\out\phase-2-graphics-direct3d-12\check
if not exist %C%\real mkdir %C%\real
if not exist %C%\ours mkdir %C%\ours
copy /y %E%\check_d3d12.exe %C%\real\ >nul
copy /y %E%\check_d3d12.exe %C%\ours\ >nul
copy /y %E%\wcontainers_d3d12.dll %C%\ours\ >nul
copy /y %E%\d3d12.dll %C%\ours\ >nul
copy /y %E%\dxgi.dll %C%\ours\ >nul
for %%w in (real ours) do (
  echo %%w:
  pushd %C%\%%w
  check_d3d12 case %1
  popd
)
