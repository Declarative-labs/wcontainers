# Waits a number of seconds and does nothing else: the jobs of tools/graphics-rig/queue-check.
param([Parameter(Mandatory)][int]$Seconds)
Start-Sleep -Seconds $Seconds
