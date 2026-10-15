param(
    [Parameter(Mandatory)][string]$PreviousExecutable,
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$ExpectedVersion,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../artifacts/app-update-acceptance')
)
$ErrorActionPreference = 'Stop'
$PreviousExecutable = (Resolve-Path -LiteralPath $PreviousExecutable).Path
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$fixture = Join-Path $OutputDirectory ('release-update-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $fixture | Out-Null
$reports = @()
foreach ($mode in @('restart-button','next-start')) {
    $directory = Join-Path $fixture $mode
    $data = Join-Path $directory 'data'
    New-Item -ItemType Directory -Force $data | Out-Null
    $executable = Join-Path $directory 'game-library-native.exe'
    Copy-Item -LiteralPath $PreviousExecutable -Destination $executable
    function Invoke-Acceptance([string[]]$NativeArguments,[string]$Name) {
        $start = [Diagnostics.ProcessStartInfo]::new($executable)
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
        $start.RedirectStandardError = $true
        foreach ($argument in @('--data-dir',$data) + $NativeArguments) { $start.ArgumentList.Add($argument) }
        $child = [Diagnostics.Process]::Start($start)
        $errors = $child.StandardError.ReadToEndAsync()
        if (!$child.WaitForExit(240000)) { $child.Kill(); throw ('Acceptance process timed out: ' + $Name) }
        $detail = $errors.GetAwaiter().GetResult()
        if ($child.ExitCode -ne 0) { throw ($Name + ': ' + $detail) }
    }
    $before = Join-Path $directory 'before.json'
    Invoke-Acceptance @('--update-command','status','--output',$before) 'Previous version status'
    $old = Get-Content -LiteralPath $before -Raw | ConvertFrom-Json
    if ([version]$old.version -ge [version]$ExpectedVersion) { throw 'Acceptance requires an older executable that already trusts the release signing key.' }
    $gameReport = Join-Path $directory 'game-before.json'
    Invoke-Acceptance @('--add-manual',$PreviousExecutable,'--title','Güncelleme kabul oyunu','--output',$gameReport) 'Seed existing library'
    $game = (Get-Content -LiteralPath $gameReport -Raw | ConvertFrom-Json).game
    $download = Join-Path $directory 'download.json'
    Invoke-Acceptance @('--update-command','download','--output',$download) 'Download real GitHub release'
    $staged = Get-Content -LiteralPath $download -Raw | ConvertFrom-Json
    if ($staged.version -ne $ExpectedVersion) { throw 'GitHub latest release differs from expected version.' }
    # A second, independent process must discover the persisted staged update.
    $persisted = Join-Path $directory 'persisted.json'
    Invoke-Acceptance @('--update-command','status','--output',$persisted) 'Persisted staged update'
    if (!(Get-Content -LiteralPath $persisted -Raw | ConvertFrom-Json).pending) { throw 'Staged update did not survive restart.' }
    if ($mode -eq 'restart-button') {
        Invoke-Acceptance @('--update-command','apply','--output',(Join-Path $directory 'requested.json')) 'Restart and update'
    } else {
        # No special apply flag: exercise automatic installation on a normal launch.
        Invoke-Acceptance @() 'Automatic installation on next start'
    }
    $resultFile = Join-Path $data 'updates/result.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    while (!(Test-Path -LiteralPath $resultFile) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    if (!(Test-Path -LiteralPath $resultFile)) { throw 'Updater did not finish; inspect isolated helper.log.' }
    $result = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
    if (!$result.success -or $result.version -ne $ExpectedVersion -or !$result.pid) { throw ('Update failed: ' + $result.message) }
    # Only close the exact candidate started by this isolated updater.
    $candidate = Get-Process -Id $result.pid -ErrorAction Stop
    if ([IO.Path]::GetFullPath($candidate.Path) -ne [IO.Path]::GetFullPath($executable)) { throw 'Unexpected candidate process; left untouched.' }
    if (!$candidate.CloseMainWindow()) { $candidate.Kill() }
    if (!$candidate.WaitForExit(10000)) { $candidate.Kill(); $candidate.WaitForExit() }
    $after = Join-Path $directory 'after.json'
    Invoke-Acceptance @('--update-command','status','--output',$after) 'Installed version status'
    $installed = Get-Content -LiteralPath $after -Raw | ConvertFrom-Json
    if ($installed.version -ne $ExpectedVersion -or $installed.pending) { throw 'Updated executable reports an unexpected version/pending update.' }
    $library = Join-Path $directory 'library-after.json'
    Invoke-Acceptance @('--list-json','--output',$library) 'Existing library after update'
    $games = (Get-Content -LiteralPath $library -Raw | ConvertFrom-Json).page.games
    if (!($games | Where-Object { $_.id -eq $game.id -and $_.name -eq $game.name })) { throw 'Existing library game did not survive update.' }
    if (!(Test-Path -LiteralPath (Join-Path $data ('updates/' + $ExpectedVersion + '/library.previous.db')))) { throw 'Update database backup missing.' }
    $upToDate = Join-Path $directory 'up-to-date.json'
    Invoke-Acceptance @('--update-command','check','--output',$upToDate) 'No update after installation'
    if ((Get-Content -LiteralPath $upToDate -Raw | ConvertFrom-Json).available) { throw 'Installed release still offers itself as an update.' }
    $reports += [ordered]@{mode=$mode;from=$old.version;to=$installed.version;githubDownload=$true;signatureVerified=$true;persisted=$true;healthy=$true;libraryPreserved=$true;databaseBackup=$true;sha256=(Get-FileHash -LiteralPath $executable).Hash;profile=$data}
}
$reports | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $fixture 'acceptance.json') -Encoding utf8
Write-Output (Join-Path $fixture 'acceptance.json')
