# Installs the page driver's packages with `npm ci` when node_modules was not installed from the
# current package-lock.json (its hash is kept in node_modules\.lock-hash).
$ErrorActionPreference = 'Stop'
. C:\wc\rig\env.ps1
$hash = (Get-FileHash "$PSScriptRoot\package-lock.json" -Algorithm SHA256).Hash
$stamp = "$PSScriptRoot\node_modules\.lock-hash"
if ((Test-Path $stamp) -and (Get-Content $stamp) -eq $hash) { exit 0 }
& 'C:\Program Files\nodejs\npm.cmd' ci --prefix $PSScriptRoot --no-fund --no-audit
if ($LASTEXITCODE) { exit $LASTEXITCODE }
Set-Content $stamp $hash
