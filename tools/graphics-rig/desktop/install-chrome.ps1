# Installs the pinned Chrome for Testing into a directory (C:\wc\chrome for the rig) with
# @puppeteer/browsers, and puts the failed-logon wrapper (gt-chrome, built by setup-desktop, path
# given) in front of it, whenever Chrome is installed or updated and whenever the wrapper there is
# not this build of it:
#   chrome-win64-real\        a tree of hard links to every file of chrome-win64 (no copies)
#   chrome-win64\chrome.exe.real  the real browser, renamed
#   chrome-win64\chrome.exe   the wrapper, which runs chrome-win64-real\chrome.exe
# Without the wrapper Chrome logs on with a blank password on every new profile, a failed logon
# (event 4625) that counts towards the account's lockout. The swap waits until no browser runs from
# chrome-win64, so none starts its children through the wrapper. Running it again changes nothing in place.
param(
    [string]$Root = 'C:\wc\chrome',
    [string]$Build = '157.0.8103.78',
    [Parameter(Mandatory)][string]$Wrapper
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
trap { [Console]::Error.WriteLine("install-chrome: $_"); exit 1 }
. $PSScriptRoot\env.ps1
if (-not (Test-Path $Wrapper)) { throw "no wrapper $Wrapper; build gt-chrome first" }
$base = "$Root\chrome\win64-$Build"
if (-not (Test-Path $base)) {
    & 'C:\Program Files\nodejs\npx.cmd' --yes '@puppeteer/browsers@3.2.7' install "chrome@$Build" --path $Root
    if ($LASTEXITCODE) { throw 'chrome download failed' }
    "installed chrome for testing $Build in $Root"
}
$src = "$base\chrome-win64"
$real = "$base\chrome-win64-real"
$wanted = (Get-FileHash $Wrapper).Hash
if ((Test-Path "$src\chrome.exe.real") -and (Get-FileHash "$src\chrome.exe").Hash -eq $wanted) {
    "the wrapper is in place in $src"
    exit 0
}
if (-not (Test-Path "$src\chrome.exe.real")) {
    if (Test-Path $real) { Remove-Item -Recurse -Force $real }
    foreach ($file in Get-ChildItem $src -Recurse -File) {
        $target = Join-Path $real $file.FullName.Substring($src.Length + 1)
        New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
        New-Item -ItemType HardLink -Path $target -Target $file.FullName | Out-Null
    }
    $count = @(Get-ChildItem $src -Recurse -File).Count
    if (@(Get-ChildItem $real -Recurse -File).Count -ne $count) { throw "the link tree $real is incomplete" }
}
$deadline = (Get-Date).AddMinutes(20)
while (@(Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.ExecutablePath -like "$src\*" }).Count) {
    if ((Get-Date) -gt $deadline) { throw "a browser from $src still runs after 20 minutes" }
    Start-Sleep -Milliseconds 500
}
if (-not (Test-Path "$src\chrome.exe.real")) { Rename-Item "$src\chrome.exe" 'chrome.exe.real' }
Copy-Item $Wrapper "$src\chrome.exe" -Force
if ((Get-FileHash "$real\chrome.exe").Hash -ne (Get-FileHash "$src\chrome.exe.real").Hash) { throw "$real\chrome.exe is not the real browser" }
"put the wrapper in front of $src\chrome.exe.real"
