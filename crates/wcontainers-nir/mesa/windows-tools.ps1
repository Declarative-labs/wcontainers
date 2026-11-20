# The build tools the Windows scripts share, dot-sourced by them: the folder everything is kept in,
# a pinned Python with Meson and Ninja, downloads with their SHA-256 printed, and the Visual C++
# compiler's environment.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Root = 'C:\wc\graphics-nir'
# Half the desktop's threads: the machine is shared with other tracks' builds.
$Jobs = [Math]::Max(1, [Environment]::ProcessorCount / 2)

$PythonVersion = '3.13.1'
$PythonUrl = "https://www.nuget.org/api/v2/package/python/$PythonVersion"
$PythonPackages = @('meson==1.12.1', 'ninja==1.13.0', 'mako==1.3.10', 'PyYAML==6.0.2', 'packaging==25.0')

$Tools = Join-Path $Root 'tools'
$Downloads = Join-Path $Tools 'downloads'
New-Item -ItemType Directory -Force -Path $Tools, $Downloads | Out-Null

function Fetch([string]$Url, [string]$Name) {
    $file = Join-Path $Downloads $Name
    if (-not (Test-Path $file)) {
        & curl.exe --fail --location --silent --show-error --output "$file.part" $Url
        if ($LASTEXITCODE -ne 0) { throw "windows-tools: could not fetch $Url" }
        Move-Item "$file.part" $file
    }
    Write-Host "windows-tools: $Name sha256 $((Get-FileHash -Algorithm SHA256 $file).Hash.ToLower())"
    $file
}

function Run([string]$What) {
    if ($LASTEXITCODE -ne 0) { throw "$What failed ($LASTEXITCODE)" }
}

# One field of a pin file's `name = "value"` lines.
function Read-PinField([string]$PinFile, [string]$Name) {
    $pattern = "^$Name *= *`"(.*)`"$"
    $line = Get-Content $PinFile | Where-Object { $_ -match $pattern } | Select-Object -First 1
    if (-not ($line -match $pattern)) { throw "windows-tools: $PinFile lacks $Name" }
    $Matches[1]
}

# A shallow checkout of one tag at its pinned commit, made again from nothing when it is another.
function Get-PinnedSource([string]$Url, [string]$Tag, [string]$Commit, [string]$Folder) {
    $head = if (Test-Path (Join-Path $Folder '.git')) { & git -C $Folder rev-parse HEAD 2>$null } else { '' }
    if ($head -eq $Commit) { return $false }
    if (Test-Path $Folder) { Remove-Item -Recurse -Force $Folder }
    & git init -q $Folder; Run 'git init'
    & git -C $Folder fetch -q --depth 1 $Url "refs/tags/$Tag"; Run 'git fetch'
    $fetched = & git -C $Folder rev-parse 'FETCH_HEAD^{commit}'
    if ($fetched -ne $Commit) { throw "windows-tools: $Tag is $fetched upstream, not the pinned $Commit" }
    & git -C $Folder -c advice.detachedHead=false checkout -q $Commit; Run 'git checkout'
    $true
}

# Python from its NuGet package (a whole Python in a zip, nothing installed), with the build's
# Python packages.
$PythonDir = Join-Path $Tools "python-$PythonVersion"
$Python = Join-Path $PythonDir 'tools\python.exe'
if (-not (Test-Path $Python)) {
    $package = Fetch $PythonUrl "python-$PythonVersion.zip"
    Expand-Archive -Path $package -DestinationPath $PythonDir
}
& $Python -m pip install --quiet --disable-pip-version-check @PythonPackages
Run 'pip install'
$env:PATH = "$(Join-Path $PythonDir 'tools\Scripts');$(Join-Path $PythonDir 'tools');$env:PATH"

# The Visual C++ compiler's environment for 64-bit x86, from the newest Visual Studio vswhere finds.
function Enter-VisualCpp {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'windows-tools: no Visual Studio with the Visual C++ tools' }
    Import-Module (Join-Path $vs 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64 -no_logo' | Out-Null
}
