param(
    [string]$SourceDirectory = (Join-Path $env:LOCALAPPDATA 'GameLibrary\integrations\vencord\source'),
    [switch]$Update,
    [string]$PluginDirectory = (Join-Path $PSScriptRoot '..\integrations\vencord\gameLibraryBridge.desktop')
)
$ErrorActionPreference = 'Stop'
$pluginDirectory = $PluginDirectory
if (!(Test-Path -LiteralPath $pluginDirectory)) { throw 'GameLibraryBridge source files are missing.' }
foreach ($tool in @('git', 'node', 'pnpm')) { if (!(Get-Command $tool -ErrorAction SilentlyContinue)) { throw "$tool is required to prepare the Vencord bridge." } }
$version = & node --version
if ([int]($version.TrimStart('v').Split('.')[0]) -lt 22) { throw 'Node.js 22 or newer is required.' }
if (!(Test-Path -LiteralPath $SourceDirectory)) {
    & git clone https://github.com/Vendicated/Vencord.git $SourceDirectory
    if ($LASTEXITCODE -ne 0) { throw 'Official Vencord checkout failed.' }
} else {
    $origin = & git -C $SourceDirectory remote get-url origin
    if ($origin -ne 'https://github.com/Vendicated/Vencord.git') { throw 'This is not the official managed Vencord checkout.' }
}
if ($Update) {
    & git -C $SourceDirectory pull --ff-only
    if ($LASTEXITCODE -ne 0) { throw 'Vencord update failed; existing files were preserved.' }
}
$destination = Join-Path $SourceDirectory 'src\userplugins\gameLibraryBridge.desktop'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Get-ChildItem -LiteralPath $pluginDirectory -Filter '*.ts' -File | Copy-Item -Destination $destination -Force
& pnpm --dir $SourceDirectory install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { throw 'Vencord dependency installation failed.' }
& pnpm --dir $SourceDirectory testTsc
if ($LASTEXITCODE -ne 0) { throw 'Bridge type check failed; Discord was not modified.' }
& pnpm --dir $SourceDirectory buildStandalone
if ($LASTEXITCODE -ne 0) { throw 'Vencord build failed; Discord was not modified.' }
$revision = & git -C $SourceDirectory rev-parse HEAD
@{ source = $SourceDirectory; revision = $revision; bridgeProtocol = 1; preparedAt = (Get-Date).ToUniversalTime().ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $SourceDirectory 'game-library-bridge.json') -Encoding utf8
Write-Output "Prepared official Vencord with GameLibraryBridge at $SourceDirectory ($revision). No Discord installation was changed."
