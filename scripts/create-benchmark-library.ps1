param(
    [ValidateRange(1, 10000)]
    [int]$Count = 1000,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $taskWorkspace ('artifacts\steam-fixture-' + $Count) }
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Hedef klasör zaten var. Yeni bir OutputDirectory seç.' }
$taskSteamApps = Join-Path $OutputDirectory 'steamapps'
New-Item -ItemType Directory -Path $taskSteamApps -Force | Out-Null
for ($taskIndex = 1; $taskIndex -le $Count; $taskIndex++) {
    $taskAppId = 1000000 + $taskIndex
    $taskName = 'Fixture Game {0:D5}' -f $taskIndex
    $taskInstallDir = 'fixture-{0:D5}' -f $taskIndex
    New-Item -ItemType Directory -Path (Join-Path $taskSteamApps ('common\' + $taskInstallDir)) -Force | Out-Null
    $taskManifest = '"AppState" {{ "appid" "{0}" "name" "{1}" "StateFlags" "4" "installdir" "{2}" "SizeOnDisk" "1073741824" }}' -f $taskAppId,$taskName,$taskInstallDir
    Set-Content -LiteralPath (Join-Path $taskSteamApps ('appmanifest_' + $taskAppId + '.acf')) -Value $taskManifest -Encoding utf8
}
Write-Output $OutputDirectory
