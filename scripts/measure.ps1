param(
    [ValidateSet('Window', 'Tray')]
    [string]$Mode = 'Window',
    [ValidateRange(10, 120)]
    [int]$Seconds = 60,
    [string]$ExecutablePath,
    [string]$DataDirectory,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
if (-not $ExecutablePath) { $ExecutablePath = Join-Path $taskWorkspace 'target\release\game-library.exe' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $taskWorkspace 'artifacts\measurements' }
$ExecutablePath = [System.IO.Path]::GetFullPath($ExecutablePath)
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (-not (Test-Path -LiteralPath $ExecutablePath)) { throw 'Release uygulaması bulunamadı. Önce pnpm build çalıştır.' }
$taskProcessName = [System.IO.Path]::GetFileNameWithoutExtension($ExecutablePath)
if (Get-Process -Name $taskProcessName -ErrorAction SilentlyContinue) { throw 'Ölçüm için önce test edilecek uygulamadan tamamen çık.' }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$taskReportPath = Join-Path $OutputDirectory ('{0}-{1}.json' -f $Mode.ToLowerInvariant(), (Get-Date -Format 'yyyyMMdd-HHmmss'))
$taskArguments = @('--benchmark', ('"{0}"' -f $taskReportPath), '--seconds', $Seconds.ToString())
if ($Mode -eq 'Tray') { $taskArguments += '--tray' }
if ($DataDirectory) { $taskArguments += @('--data-dir', ('"{0}"' -f [System.IO.Path]::GetFullPath($DataDirectory))) }
$taskProcess = Start-Process -FilePath $ExecutablePath -ArgumentList $taskArguments -WindowStyle Hidden -PassThru
$taskProcess.WaitForExit()
if ($taskProcess.ExitCode -ne 0) { throw ('Ölçüm uygulaması hata koduyla çıktı: {0}' -f $taskProcess.ExitCode) }
if (-not (Test-Path -LiteralPath $taskReportPath)) { throw 'Ölçüm raporu oluşturulmadı.' }
$taskReport = Get-Content -Raw -LiteralPath $taskReportPath | ConvertFrom-Json
$taskComputer = Get-CimInstance Win32_ComputerSystem
$taskSystem = Get-CimInstance Win32_OperatingSystem
$taskCpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$taskReport | Add-Member -NotePropertyName scenario -NotePropertyValue $Mode
$taskReport | Add-Member -NotePropertyName machine -NotePropertyValue ([pscustomobject]@{
    os = $taskSystem.Caption
    osBuild = $taskSystem.BuildNumber
    cpu = $taskCpu.Name
    totalRamGiB = [math]::Round($taskComputer.TotalPhysicalMemory / 1GB, 2)
})
$taskReport | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $taskReportPath -Encoding utf8
[pscustomobject]@{
    Mode = $Mode
    Seconds = $taskReport.durationSeconds
    PrivateRamMiB = $(if ($null -ne $taskReport.averagePrivateWorkingSetMib) { [math]::Round($taskReport.averagePrivateWorkingSetMib, 2) } else { 'Unavailable' })
    PeakPrivateRamMiB = $(if ($null -ne $taskReport.peakPrivateWorkingSetMib) { [math]::Round($taskReport.peakPrivateWorkingSetMib, 2) } else { 'Unavailable' })
    PrivateBytesMiB = [math]::Round($taskReport.averagePrivateBytesMib, 2)
    CpuPercent = $(if ($null -ne $taskReport.averageCpuPercent) { [math]::Round($taskReport.averageCpuPercent, 3) } else { 'Unavailable' })
    Partial = $taskReport.partial
    Report = $taskReportPath
} | Format-List
