# Installs what the rig needs on the desktop, each step skipped when already done: Git, Node, the
# Visual Studio 2022 C++ build tools with their Windows SDK, Rust (rustup in C:\wc), Windows' Graphics
# Tools feature (the Direct3D debug layers), pueue, and Chrome for Testing in C:\wc\chrome.
# Versions are pinned here, except the Visual Studio bootstrapper, which always installs Microsoft's
# current 17.x build tools; the versions present are printed at the end and written to
# C:\wc\setup\versions.txt. The status file lists what each install changed.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("install-tools: $_"); exit 1 }
. $PSScriptRoot\env.ps1
# The workspace's rust-toolchain.toml channel, so one toolchain serves the checkouts and the default.
$rustToolchain = '1.99.0'
$chromeBuild = '157.0.8103.78'
$d = 'C:\wc\downloads'
New-Item -ItemType Directory -Force $d, C:\wc\bin, C:\wc\setup | Out-Null
function Step($m) { "$(Get-Date -Format s) $m" }
function Download($url, $name) {
    $f = Join-Path $d $name
    if (-not (Test-Path $f)) { curl.exe -sSfL -o $f $url; if ($LASTEXITCODE) { throw "download $url" } }
    $f
}

$git = Download 'https://github.com/git-for-windows/git/releases/download/v2.56.1.windows.1/Git-2.56.1-64-bit.exe' 'Git-2.56.1-64-bit.exe'
$node = Download 'https://nodejs.org/dist/v26.13.1/node-v26.13.1-x64.msi' 'node-v26.13.1-x64.msi'
$vs = Download 'https://aka.ms/vs/17/release/vs_BuildTools.exe' 'vs_BuildTools.exe'
$rust = Download 'https://static.rust-lang.org/rustup/archive/1.29.3/x86_64-pc-windows-msvc/rustup-init.exe' 'rustup-init-1.29.3.exe'
Download 'https://github.com/Nukesor/pueue/releases/download/v4.0.4/pueue-x86_64-pc-windows-msvc.exe' 'pueue.exe' | Out-Null
Download 'https://github.com/Nukesor/pueue/releases/download/v4.0.4/pueued-x86_64-pc-windows-msvc.exe' 'pueued.exe' | Out-Null
if (-not (Test-Path C:\wc\bin\pueued.exe)) { Copy-Item "$d\pueue.exe", "$d\pueued.exe" C:\wc\bin }

if (-not (Test-Path 'C:\Program Files\Git\cmd\git.exe')) {
    Step 'git'
    # Only Git LFS among the optional parts: no shell menu entries, icons or file associations.
    Start-Process $git -Wait -ArgumentList '/VERYSILENT', '/NORESTART', '/NOCANCEL', '/SP-', '/SUPPRESSMSGBOXES', '/COMPONENTS=gitlfs'
}
if (-not (Test-Path 'C:\Program Files\nodejs\node.exe')) {
    Step 'node'
    $p = Start-Process msiexec.exe -Wait -PassThru -ArgumentList '/i', $node, '/qn', '/norestart'
    if ($p.ExitCode) { throw "node installer exit $($p.ExitCode)" }
}
if (-not (Test-Path 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC')) {
    Step 'visual studio build tools'
    $p = Start-Process $vs -Wait -PassThru -ArgumentList '--quiet', '--wait', '--norestart', '--nocache', '--add', 'Microsoft.VisualStudio.Workload.VCTools', '--includeRecommended'
    if ($p.ExitCode) { throw "build tools installer exit $($p.ExitCode)" }
}
[Environment]::SetEnvironmentVariable('RUSTUP_HOME', 'C:\wc\rustup', 'User')
[Environment]::SetEnvironmentVariable('CARGO_HOME', 'C:\wc\cargo', 'User')
$env:RUSTUP_HOME = 'C:\wc\rustup'
$env:CARGO_HOME = 'C:\wc\cargo'
if (-not (Test-Path 'C:\wc\cargo\bin\cargo.exe')) {
    Step 'rust'
    $p = Start-Process $rust -Wait -PassThru -NoNewWindow -ArgumentList '-y', '--default-toolchain', $rustToolchain, '--profile', 'minimal', '--default-host', 'x86_64-pc-windows-msvc'
    if ($p.ExitCode) { throw "rustup-init exit $($p.ExitCode)" }
}
$rustup = 'C:\wc\cargo\bin\rustup.exe'
& $rustup default $rustToolchain | Out-Null
if (& $rustup toolchain list | Select-String '^stable') { & $rustup toolchain uninstall stable | Out-Null }
# Adding a Windows feature is refused to an SSH logon, so a one-off task does it as SYSTEM.
if ((Get-WindowsCapability -Online -Name 'Tools.Graphics.DirectX~~~~0.0.1.0').State -ne 'Installed') {
    Step 'graphics tools'
    $task = 'wc-graphics-tools-once'
    $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -Command `"Add-WindowsCapability -Online -Name 'Tools.Graphics.DirectX~~~~0.0.1.0' *> C:\wc\setup\graphics-tools.log`""
    Register-ScheduledTask -TaskName $task -Action $action -User 'SYSTEM' -RunLevel Highest -Force | Out-Null
    Start-ScheduledTask -TaskName $task
    do { Start-Sleep 5 } while ((Get-ScheduledTask -TaskName $task).State -eq 'Running')
    Unregister-ScheduledTask -TaskName $task -Confirm:$false
    if ((Get-WindowsCapability -Online -Name 'Tools.Graphics.DirectX~~~~0.0.1.0').State -ne 'Installed') { throw 'Graphics Tools did not install; see C:\wc\setup\graphics-tools.log' }
}
if (-not (Test-Path "C:\wc\chrome\chrome\win64-$chromeBuild")) {
    Step 'chrome for testing'
    & 'C:\Program Files\nodejs\npx.cmd' --yes '@puppeteer/browsers@3.2.7' install "chrome@$chromeBuild" --path C:\wc\chrome
    if ($LASTEXITCODE) { throw 'chrome download failed' }
}
$vs = & 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe' -products * -property catalog_productDisplayVersion
$msvc = (Get-ChildItem 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC').Name -join ' '
$sdk = (Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\Include').Name -join ' '
$versions = @(
    "git $((& 'C:\Program Files\Git\cmd\git.exe' --version) -replace 'git version ', '')"
    "node $(& 'C:\Program Files\nodejs\node.exe' --version)"
    "visual studio build tools $vs, msvc $msvc, windows sdk $sdk"
    "rustup $((cmd /c "$rustup --version 2>nul") -replace 'rustup ', ''), default toolchain $(& $rustup default)"
    "$(& C:\wc\bin\pueue.exe --version)"
    "chrome for testing $((Get-ChildItem C:\wc\chrome\chrome -Directory).Name -join ' ')"
)
$versions | Set-Content C:\wc\setup\versions.txt
$versions
Step 'tools in place'
