param(
    [ValidateRange(1, 10000)]
    [int]$Count = 1000,
    [string]$OutputDirectory,
    [switch]$WithArtwork
)

$ErrorActionPreference = 'Stop'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $taskWorkspace ('artifacts\steam-fixture-' + $Count) }
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Hedef klasör zaten var. Yeni bir OutputDirectory seç.' }
$taskSteamApps = Join-Path $OutputDirectory 'steamapps'
New-Item -ItemType Directory -Path $taskSteamApps -Force | Out-Null
if ($WithArtwork) {
    Add-Type -AssemblyName System.Drawing
    $taskArtworkDirectory = Join-Path $OutputDirectory 'appcache\librarycache'
    New-Item -ItemType Directory -Path $taskArtworkDirectory -Force | Out-Null
    $taskBitmap = [System.Drawing.Bitmap]::new(600, 900)
    $taskGraphics = [System.Drawing.Graphics]::FromImage($taskBitmap)
    $taskFont = [System.Drawing.Font]::new('Segoe UI', 26)
    $taskBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(199, 218, 189))
}
try {
    for ($taskIndex = 1; $taskIndex -le $Count; $taskIndex++) {
        $taskAppId = 1000000 + $taskIndex
        $taskName = 'Fixture Game {0:D5}' -f $taskIndex
        $taskInstallDir = 'fixture-{0:D5}' -f $taskIndex
        New-Item -ItemType Directory -Path (Join-Path $taskSteamApps ('common\' + $taskInstallDir)) -Force | Out-Null
        $taskManifest = '"AppState" {{ "appid" "{0}" "name" "{1}" "StateFlags" "4" "installdir" "{2}" "SizeOnDisk" "1073741824" }}' -f $taskAppId,$taskName,$taskInstallDir
        Set-Content -LiteralPath (Join-Path $taskSteamApps ('appmanifest_' + $taskAppId + '.acf')) -Value $taskManifest -Encoding utf8
        if ($WithArtwork) {
            $taskGraphics.Clear([System.Drawing.Color]::FromArgb(24 + ($taskIndex % 20), 36 + ($taskIndex % 25), 40 + ($taskIndex % 30)))
            $taskGraphics.DrawString(('BENCHMARK' + [Environment]::NewLine + $taskName), $taskFont, $taskBrush, [System.Drawing.RectangleF]::new(42, 330, 516, 420))
            $taskBitmap.Save((Join-Path $taskArtworkDirectory ($taskAppId.ToString() + '_library_600x900.jpg')), [System.Drawing.Imaging.ImageFormat]::Jpeg)
        }
    }
} finally {
    if ($WithArtwork) { $taskBrush.Dispose(); $taskFont.Dispose(); $taskGraphics.Dispose(); $taskBitmap.Dispose() }
}
Write-Output $OutputDirectory
