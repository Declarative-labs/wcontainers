# Dot-sourced by the rig's desktop scripts: keeps the temporary files, npm's cache and Node's compile
# cache of everything the rig runs under C:\wc, out of the owner's profile.
New-Item -ItemType Directory -Force C:\wc\tmp, C:\wc\npm-cache | Out-Null
$env:TEMP = 'C:\wc\tmp'
$env:TMP = 'C:\wc\tmp'
$env:npm_config_cache = 'C:\wc\npm-cache'
$env:NODE_COMPILE_CACHE = 'C:\wc\tmp\node-compile-cache'
