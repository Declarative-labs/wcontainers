# What is installed and set on the desktop, for comparing with what the rig's status file lists:
# installed programs, scheduled tasks at the root, services, the Graphics Tools feature, the
# machine's and the user's PATH and the user's Rust variables, and what is in C:\wc.
$ProgressPreference = 'SilentlyContinue'
'== programs'
Get-ItemProperty HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*, HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*, HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue |
    Where-Object DisplayName | Sort-Object DisplayName | ForEach-Object { "$($_.DisplayName) | $($_.DisplayVersion)" }
'== scheduled tasks'
Get-ScheduledTask | Where-Object TaskPath -eq '\' | ForEach-Object TaskName
'== services'
Get-Service | Sort-Object Name | ForEach-Object { "$($_.Name) $($_.StartType)" }
'== graphics tools'
(Get-WindowsCapability -Online -Name 'Tools.Graphics.DirectX~~~~0.0.1.0').State
'== machine path'
[Environment]::GetEnvironmentVariable('Path', 'Machine')
'== user path and variables'
[Environment]::GetEnvironmentVariable('Path', 'User')
"RUSTUP_HOME=$([Environment]::GetEnvironmentVariable('RUSTUP_HOME', 'User'))"
"CARGO_HOME=$([Environment]::GetEnvironmentVariable('CARGO_HOME', 'User'))"
'== C:\wc'
Get-ChildItem C:\wc | ForEach-Object Name
