param(
    [Parameter(Mandatory)][ValidatePattern('^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$')][string]$Tag,
    [string]$Executable = (Join-Path $PSScriptRoot '../target/release/game-library-native.exe'),
    [string]$VencordSource = (Join-Path $env:LOCALAPPDATA 'GameLibrary/integrations/vencord/source'),
    [string]$OutputDirectory,
    [string]$PrivateKeyFile = '-',
    [switch]$Publish
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$version = $Tag.Substring(1)
$cargoVersion = ([regex]::Match([IO.File]::ReadAllText((Join-Path $repo 'apps/native/Cargo.toml')), '(?m)^version = "([^"]+)"')).Groups[1].Value
if ($version -ne $cargoVersion) { throw 'Tag and native application versions must match.' }
if (& git -C $repo status --porcelain) { throw 'Release sources must be committed and clean.' }
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repo ('artifacts/releases/' + $Tag) }
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$Executable = (Resolve-Path -LiteralPath $Executable).Path
& (Join-Path $PSScriptRoot 'package-windows-test.ps1') -Executable $Executable -VencordSource $VencordSource -OutputDirectory $OutputDirectory
if ($LASTEXITCODE -ne 0) { throw 'Packaging failed.' }
Copy-Item -LiteralPath $Executable -Destination (Join-Path $OutputDirectory 'game-library-native.exe')
& node (Join-Path $PSScriptRoot 'sign-update.mjs') $Executable $version $PrivateKeyFile (Join-Path $OutputDirectory 'update.json')
if ($LASTEXITCODE -ne 0) { throw 'Release signing failed.' }
$start = [Diagnostics.ProcessStartInfo]::new($Executable)
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
foreach ($argument in @('--data-dir',(Join-Path $OutputDirectory 'verify-profile'),'--update-command','verify','--folder',$OutputDirectory,'--output',(Join-Path $OutputDirectory 'verified.json'))) { $start.ArgumentList.Add($argument) }
$child = [Diagnostics.Process]::Start($start)
if (!$child.WaitForExit(60000) -or $child.ExitCode -ne 0) { throw 'Native signature/binary verification failed.' }
$verified = Get-Content -LiteralPath (Join-Path $OutputDirectory 'verified.json') -Raw | ConvertFrom-Json
if ($verified.version -ne $version) { throw 'Verified package has a different version.' }
# A real startup report binds the executable version to the signed manifest.
$start.ArgumentList.Clear()
$healthToken = [Guid]::NewGuid().ToString('N')
foreach ($argument in @('--data-dir',(Join-Path $OutputDirectory 'startup-profile'),'--startup-report',(Join-Path $OutputDirectory 'startup.json'),'--update-health-report',(Join-Path $OutputDirectory 'health.json'),'--update-health-token',$healthToken)) { $start.ArgumentList.Add($argument) }
$child = [Diagnostics.Process]::Start($start)
if (!$child.WaitForExit(60000) -or $child.ExitCode -ne 0) { throw 'Release UI startup failed.' }
$startup = Get-Content -LiteralPath (Join-Path $OutputDirectory 'startup.json') -Raw | ConvertFrom-Json
$health = Get-Content -LiteralPath (Join-Path $OutputDirectory 'health.json') -Raw | ConvertFrom-Json
if (!$startup.rendered -or !$health.ready -or $health.version -ne $version -or $health.pid -ne $child.Id -or $health.token -ne $healthToken) { throw 'Executable startup health/version differs from manifest.' }
if ($Publish) {
    $tagCommit = & git -C $repo rev-list -n 1 $Tag
    if ($LASTEXITCODE -ne 0 -or $tagCommit -ne (& git -C $repo rev-parse HEAD)) { throw 'Release tag must point to the exact packaged commit.' }
    $releaseRepo = 'tugcantopaloglu/game-library'
    $existing = & gh release view $Tag --repo $releaseRepo --json isDraft 2>$null
    if ($LASTEXITCODE -eq 0) {
        if (!(($existing | ConvertFrom-Json).isDraft)) { throw 'Published releases are immutable; create a new version.' }
    } else {
        & gh release create $Tag --repo $releaseRepo --draft --verify-tag --title ('Game Library ' + $version) --notes-file (Join-Path $repo ('release/' + $Tag + '.md'))
        if ($LASTEXITCODE -ne 0) { throw 'Draft release creation failed.' }
    }
    $assets = @((Join-Path $OutputDirectory 'game-library-native.exe'),(Join-Path $OutputDirectory 'update.json'),(Join-Path $OutputDirectory ('GameLibrary-Windows-x64-' + $version + '.zip')),(Join-Path $OutputDirectory 'GameLibrary/Sources/vencord-source.zip'))
    & gh release upload $Tag @assets --repo $releaseRepo --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Release assets were not all uploaded; draft remains unpublished.' }
    & gh release edit $Tag --repo $releaseRepo --draft=false --latest
    if ($LASTEXITCODE -ne 0) { throw 'Release publication failed.' }
}
Write-Output ('Verified signed release: ' + $OutputDirectory)
