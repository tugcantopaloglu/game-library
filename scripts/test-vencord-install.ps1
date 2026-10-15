param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\release\game-library-native.exe'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\artifacts\bridge-auto-install-20261015')
)
$ErrorActionPreference = 'Stop'
if (Get-Process -Name Discord,DiscordPTB,DiscordCanary -ErrorAction SilentlyContinue) {
    throw 'Close Discord before running the isolated official installer test. The test will not close it for you.'
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$fixture = Join-Path $OutputDirectory ('installer-fixture-' + [Guid]::NewGuid().ToString('N'))
$fixtureLocal = Join-Path $fixture 'local'
$fixtureRoaming = Join-Path $fixture 'roaming'
$fixtureData = Join-Path $fixture 'data'
$resources = Join-Path $fixtureLocal 'Discord\app-1.0.0\resources'
New-Item -ItemType Directory -Path $resources -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $resources 'app.asar'), 'isolated original Discord fixture')
$originalHash = (Get-FileHash -LiteralPath (Join-Path $resources 'app.asar')).Hash

function Invoke-Fixture([string[]]$NativeArguments, [string]$LogName) {
    $start = [Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['LOCALAPPDATA'] = $fixtureLocal
    $start.Environment['APPDATA'] = $fixtureRoaming
    foreach ($argument in @('--data-dir', $fixtureData) + $NativeArguments) { $start.ArgumentList.Add($argument) }
    $child = [Diagnostics.Process]::Start($start)
    $stdout = $child.StandardOutput.ReadToEndAsync()
    $stderr = $child.StandardError.ReadToEndAsync()
    if (!$child.WaitForExit(300000)) { $child.Kill(); throw 'Isolated installer fixture timed out.' }
    $standardOutput = $stdout.GetAwaiter().GetResult()
    $standardError = $stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $fixture ($LogName + '.stdout.log')), $standardOutput)
    [IO.File]::WriteAllText((Join-Path $fixture ($LogName + '.stderr.log')), $standardError)
    return [pscustomobject]@{ ExitCode = $child.ExitCode; Output = $standardOutput; Error = $standardError }
}

Write-Output 'Testing verified official installer against an isolated Discord fixture...'
$freshReport = Join-Path $fixture 'fresh.json'
$fresh = Invoke-Fixture @('--install-bridge', '--output', $freshReport) 'fresh'
if ($fresh.ExitCode -ne 0) { throw ('Fresh fixture install failed: ' + $fresh.Error) }
$freshResult = Get-Content -LiteralPath $freshReport -Raw | ConvertFrom-Json
if (!$freshResult.changed) { throw 'Fresh fixture was not installed.' }
if ((Get-Content -LiteralPath (Join-Path $fixtureRoaming 'Vencord\dist\package.json') -Raw | ConvertFrom-Json).type -ne 'commonjs') { throw 'Electron CommonJS package boundary is missing.' }
if ((Get-FileHash -LiteralPath (Join-Path $resources '_app.asar')).Hash -ne $originalHash) { throw 'Original Discord archive was not preserved.' }
$loaderHash = (Get-FileHash -LiteralPath (Join-Path $resources 'app.asar')).Hash
$installerLog = Join-Path $fixtureData 'integrations\vencord\installer.log'
$installerLogTime = (Get-Item -LiteralPath $installerLog).LastWriteTimeUtc

Write-Output 'Testing idempotent re-install and offline bundle repair...'
$repeatReport = Join-Path $fixture 'repeat.json'
$repeat = Invoke-Fixture @('--install-bridge', '--output', $repeatReport) 'repeat'
if ($repeat.ExitCode -ne 0 -or (Get-Content -LiteralPath $repeatReport -Raw | ConvertFrom-Json).changed) { throw 'Repeated install was not idempotent.' }
$renderer = Join-Path $fixtureRoaming 'Vencord\dist\renderer.js'
$rendererHash = (Get-FileHash -LiteralPath $renderer).Hash
[IO.File]::WriteAllText($renderer, 'simulated stale bridge')
$repair = Invoke-Fixture @('--install-bridge', '--output', (Join-Path $fixture 'repair.json')) 'repair'
if ($repair.ExitCode -ne 0 -or (Get-FileHash -LiteralPath $renderer).Hash -ne $rendererHash) { throw 'Embedded bundle repair failed.' }
if ((Get-Item -LiteralPath $installerLog).LastWriteTimeUtc -ne $installerLogTime) { throw 'A normal bridge update unnecessarily ran the official installer.' }
if ((Get-FileHash -LiteralPath (Join-Path $resources 'app.asar')).Hash -ne $loaderHash) { throw 'A normal update changed the Discord loader.' }

Write-Output 'Testing settings failure without modifying installed files...'
$settings = Join-Path $fixtureRoaming 'Vencord\settings\settings.json'
$validSettings = [IO.File]::ReadAllBytes($settings)
[IO.File]::WriteAllText($settings, '{invalid fixture settings')
$failure = Invoke-Fixture @('--install-bridge') 'invalid-settings'
if ($failure.ExitCode -eq 0) { throw 'Invalid settings were accepted.' }
if ((Get-FileHash -LiteralPath $renderer).Hash -ne $rendererHash) { throw 'Invalid settings modified the installed bridge.' }
[IO.File]::WriteAllBytes($settings, $validSettings)

$report = [ordered]@{
    fixture = $fixture
    verifiedOfficialInstaller = $true
    originalDiscordArchivePreserved = $true
    freshInstallPassed = $true
    commonJsBoundaryPassed = $true
    idempotentInstallPassed = $true
    embeddedRepairWithoutInstallerPassed = $true
    invalidSettingsPreservedInstallation = $true
    realDiscordSessionTested = $false
    bridgeBuild = $freshResult.build
    executableSha256 = (Get-FileHash -LiteralPath $Executable).Hash
}
$report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'official-installer-acceptance.json') -Encoding utf8
$report | ConvertTo-Json
