@echo off
rem The own program, run with real Direct3D 12 and through our libraries, compared.
set E=%CARGO_TARGET_DIR%\debug\examples
set C=C:\wc\out\phase-2-graphics-direct3d-12\check
if not exist %C%\real mkdir %C%\real
if not exist %C%\ours mkdir %C%\ours
copy /y %E%\check_d3d12.exe %C%\real\ >nul
copy /y %E%\check_d3d12.exe %C%\ours\ >nul
copy /y %E%\d3d12.dll %C%\ours\ >nul
copy /y %E%\dxgi.dll %C%\ours\ >nul
for %%w in (real ours) do (
  pushd %C%\%%w
  check_d3d12 draw draw.rgba
  check_d3d12 features features.txt
  check_d3d12 outputs outputs.txt
  echo fence-wait %%w:
  check_d3d12 fence-wait 2000
  popd
)
echo draw:
%C%\real\check_d3d12 compare %C%\real\draw.rgba %C%\ours\draw.rgba 1
echo features:
%C%\real\check_d3d12 compare-features %C%\real\features.txt %C%\ours\features.txt
echo outputs:
%C%\real\check_d3d12 compare-features %C%\real\outputs.txt %C%\ours\outputs.txt all
