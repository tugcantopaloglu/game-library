param(
    [ValidateRange(1, 10000)] [int]$EpicCount = 1000,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $taskWorkspace 'artifacts\source-fixtures' }
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Hedef klasör zaten var. Yeni bir OutputDirectory seç.' }
$taskEpic = Join-Path $OutputDirectory 'Epic\Data\Manifests'
New-Item -ItemType Directory -Path $taskEpic -Force | Out-Null
for ($taskIndex = 1; $taskIndex -le $EpicCount; $taskIndex++) {
    $taskInstall = Join-Path $OutputDirectory ('Epic Games\fixture-' + $taskIndex)
    New-Item -ItemType Directory -Path $taskInstall -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $taskInstall 'Game.exe'), 'Fixture only; not executable.')
    $taskManifest = [ordered]@{
        AppName = 'FixtureEpic' + $taskIndex
        CatalogNamespace = 'fixture-namespace'
        CatalogItemId = 'fixture-catalog-' + $taskIndex
        DisplayName = 'Z Fixture Epic {0:D5}' -f $taskIndex
        InstallLocation = $taskInstall
        LaunchExecutable = 'Game.exe'
        MainGameAppName = 'FixtureEpic' + $taskIndex
        bIsApplication = $true
        bIsExecutable = $true
        bIsIncompleteInstall = $false
        InstallSize = '1073741824'
        AppCategories = @('games')
    }
    $taskManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskEpic ($taskIndex.ToString() + '.item')) -Encoding utf8
}
$taskRiot = Join-Path $OutputDirectory 'Riot Data'
$taskClientDirectory = Join-Path $OutputDirectory 'Mock Riot Client'
New-Item -ItemType Directory -Path $taskClientDirectory -Force | Out-Null
$taskClient = Join-Path $taskClientDirectory 'RiotClientServices.exe'
& rustc (Join-Path $taskWorkspace 'fixtures\riot\mock-client.rs') -O -C panic=abort -o $taskClient
if ($LASTEXITCODE -ne 0) { throw 'Mock Riot Client derlenemedi.' }
$taskAssociated = [ordered]@{}
foreach ($taskProduct in @(@{id='league_of_legends';exe='LeagueClient.exe'}, @{id='valorant';exe='VALORANT.exe'}, @{id='bacon';exe='LoR.exe'}, @{id='lion';exe='Lion.exe'})) {
    $taskName = $taskProduct.id + '.live'
    $taskInstall = Join-Path $OutputDirectory ('Riot Games\' + $taskProduct.id)
    $taskMetadata = Join-Path $taskRiot ('Metadata\' + $taskName)
    New-Item -ItemType Directory -Path $taskInstall,$taskMetadata -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $taskInstall $taskProduct.exe), 'Fixture only; not executable.')
    $taskYamlInstall = $taskInstall.Replace("'", "''")
    $taskYamlRoot = $OutputDirectory.Replace("'", "''")
    $taskYaml = @("product_install_full_path: '$taskYamlInstall'", "product_install_root: '$taskYamlRoot'", 'product_install_enabled: true') -join "`n"
    Set-Content -LiteralPath (Join-Path $taskMetadata ($taskName + '.product_settings.yaml')) -Value $taskYaml -Encoding utf8
    $taskAssociated[$taskInstall] = $taskClient
}
[ordered]@{rc_default=$taskClient;associated_client=$taskAssociated} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskRiot 'RiotClientInstalls.json') -Encoding utf8
[pscustomobject]@{EpicManifests=$taskEpic;RiotData=$taskRiot;MockClient=$taskClient;ExpectedEpic=$EpicCount;ExpectedRiot=4}
