# What the conformance scripts share, dot-sourced by them: where the suite and its tools are, a
# folder per driver to run glcts from, and a list's configuration from its mustpass.xml.
. (Join-Path $PSScriptRoot '..\..\wcontainers-nir\mesa\windows-tools.ps1')

$Cts = Join-Path $Root 'cts'
$Mustpass = Join-Path $Cts 'src\external\openglcts\data\gl_cts\data\mustpass'
$GlctsDir = Join-Path $Cts 'build\external\openglcts\modules'
$Glcts = Join-Path $GlctsDir 'glcts.exe'
$Runner = Join-Path $Tools 'deqp-runner\bin\deqp-runner.exe'
$MesaBin = Join-Path $Root 'mesa\install\bin'
$ListsFile = Join-Path $PSScriptRoot 'lists.txt'
# The driver name that runs the system's OpenGL (the GPU vendor's) instead of Mesa's.
$System = 'system'

# Windows loads opengl32.dll from the program's own folder first, so each driver runs a hard link
# of glcts in a folder of its own: Mesa's DLLs beside it, or none for the system's OpenGL, with the
# suite's data read from the build folder. Returns the program to run.
function New-DriverFolder([string]$Driver) {
    if (-not (Test-Path $Glcts)) { throw "cts: $Glcts is missing; run cts\build-windows.ps1" }
    $folder = Join-Path $Cts "run\$Driver"
    if (Test-Path $folder) { Remove-Item -Recurse -Force $folder }
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    $program = Join-Path $folder (Split-Path -Leaf $Glcts)
    New-Item -ItemType HardLink -Path $program -Value $Glcts | Out-Null
    if ($Driver -ne $System) {
        foreach ($library in Get-ChildItem (Join-Path $MesaBin '*') -Include '*.dll', '*.pdb') {
            Copy-Item -Force $library.FullName $folder
        }
        $env:GALLIUM_DRIVER = $Driver
    }
    $program
}

# The lists of lists.txt, as paths under the mustpass folder.
function Get-Lists {
    Get-Content $ListsFile | Where-Object { $_ -and -not $_.StartsWith('#') }
}

# The CTS's own arguments for a list: its first configuration in its folder's mustpass.xml, with
# the folder glcts reads its data from. The ES lists keep glcts's default, stopping on a lost
# context: glcts asks for a context told of resets and asks after each case, and the driver
# reports a context one of whose calls it refused, or whose device was lost, as reset by its own
# fault, so that case ends the process and deqp-runner runs the rest in a new one. The desktop
# lists turn it off: glcts's desktop packages call glGetGraphicsResetStatus after every case, an
# entry point they load only from GL_KHR_robustness or OpenGL 4.5, so a 3.3 context without it
# calls a null pointer (glcts gl3cTestPackages.cpp, TestCaseWrapper::iterate).
$KeepRunningOnDeviceLost = '--deqp-terminate-on-device-lost=disable'
$DesktopLists = 'gl/'
function Get-ListArguments([string]$List) {
    $caselist = Join-Path $Mustpass $List
    [xml]$described = Get-Content (Join-Path (Split-Path $caselist) 'mustpass.xml')
    $configuration = $described.SelectNodes('//Configuration') |
        Where-Object { $_.caseListFile -eq (Split-Path -Leaf $List) } | Select-Object -First 1
    if (-not $configuration) { throw "cts: mustpass.xml has no configuration for $List" }
    $arguments = @($configuration.commandLine -split ' ' | Where-Object { $_ }) + "--deqp-archive-dir=$GlctsDir"
    if ($List.StartsWith($DesktopLists)) { $arguments += $KeepRunningOnDeviceLost }
    $arguments
}
