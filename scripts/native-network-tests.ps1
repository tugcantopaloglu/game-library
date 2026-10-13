# Offline tests only. DNS functions below are replaced with in-memory adapters;
# no UAC, driver, live DNS modification, service or public-network call occurs.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'native-network.ps1') -LibraryOnly
$script:NetworkTestCount = 0
function Assert-NetworkTest([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw $Message }
    $script:NetworkTestCount++
}
function Assert-NetworkThrows([scriptblock] $Action, [string] $Message) {
    $threw = $false
    try { & $Action | Out-Null } catch { $threw = $true }
    Assert-NetworkTest $threw $Message
}
function New-NetworkTestZip([string] $Path, [string[]] $Names) {
    $zip = [IO.Compression.ZipFile]::Open($Path, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $Names) {
            $stream = $zip.CreateEntry($name).Open()
            try { $bytes = [Text.Encoding]::ASCII.GetBytes('MZnetwork-test-only'); $stream.Write($bytes,0,$bytes.Length) }
            finally { $stream.Dispose() }
        }
    } finally { $zip.Dispose() }
}

$taskTestRoot = Join-Path ([IO.Path]::GetTempPath()) ('game-library-network-tests-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($taskTestRoot) | Out-Null
try {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    foreach ($name in @('../escape.exe','x/../../escape.exe','C:/escape.exe','/escape.exe','x\escape.exe','x/evil:stream','x/CON.txt','x/NUL','x/file.','x//file')) {
        Assert-NetworkTest (-not (Test-SafeZipName $name)) ('Unsafe ZIP name accepted: ' + $name)
    }
    Assert-NetworkTest (Test-SafeZipName 'goodbyedpi-0.2.2/x86_64/goodbyedpi.exe') 'Valid package path rejected.'
    $badArchive = Join-Path $taskTestRoot 'bad.zip'
    New-NetworkTestZip $badArchive @('../escape.exe')
    Assert-NetworkThrows { Expand-SafePackage $badArchive (Join-Path $taskTestRoot 'bad-output') } 'Traversal archive accepted.'
    Assert-NetworkTest (-not (Test-Path -LiteralPath (Join-Path $taskTestRoot 'bad-output'))) 'Invalid archive wrote a staging directory.'
    Assert-NetworkTest (-not (Test-Path -LiteralPath (Join-Path $taskTestRoot 'escape.exe'))) 'Traversal created an external file.'
    $duplicates = Join-Path $taskTestRoot 'duplicates.zip'
    New-NetworkTestZip $duplicates @('x/File.txt','x/file.txt')
    Assert-NetworkThrows { Expand-SafePackage $duplicates (Join-Path $taskTestRoot 'duplicate-output') } 'Case-insensitive duplicate accepted.'
    $safeArchive = Join-Path $taskTestRoot 'safe.zip'
    New-NetworkTestZip $safeArchive @('goodbyedpi-0.2.2/x86_64/goodbyedpi.exe','goodbyedpi-0.2.2/x86_64/WinDivert.dll','goodbyedpi-0.2.2/x86_64/WinDivert64.sys')
    Expand-SafePackage $safeArchive (Join-Path $taskTestRoot 'safe-output')
    Assert-NetworkTest (Test-Path -LiteralPath (Join-Path $taskTestRoot 'safe-output/goodbyedpi-0.2.2/x86_64/WinDivert64.sys')) 'Safe archive did not extract.'
    Assert-NetworkThrows { Assert-ChildPath $taskTestRoot ([IO.Path]::GetFullPath((Join-Path $taskTestRoot '../outside'))) } 'Outside target accepted.'

    # ACL descriptors are checked in memory. Protected writes below are mocked
    # into our temporary fixture, so this test never changes live system ACLs.
    $script:CallerSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $descriptor = New-ProtectedDirectoryAcl $script:CallerSid
    Assert-NetworkTest ($descriptor.GetOwner([Security.Principal.SecurityIdentifier]).Value -eq 'S-1-5-32-544') 'Protected owner is not Administrators.'
    $readerRule = @($descriptor.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]) | Where-Object {$_.IdentityReference.Value -eq $script:CallerSid})
    Assert-NetworkTest (($readerRule[0].FileSystemRights -band [Security.AccessControl.FileSystemRights]::Write) -eq 0) 'Reader has protected write rights.'
    function New-ProtectedDirectory([string] $Path, [string] $ReaderSid) { [IO.Directory]::CreateDirectory($Path) | Out-Null }
    function Assert-ProtectedAcl([string] $Path) { return (New-ProtectedDirectoryAcl $script:CallerSid) }
    function Write-ProtectedBytes([string] $Path, [byte[]] $Bytes) { [IO.File]::WriteAllBytes($Path,$Bytes) }
    function Write-ProtectedJson([string] $Path, $Value) { Write-Json $Path $Value }
    $script:OfficialBytes = [IO.File]::ReadAllBytes($safeArchive)
    $script:OfficialFetchCount = 0
    function Read-OfficialAsset([string] $Address, [long] $Limit) { $script:OfficialFetchCount++; return ,$script:OfficialBytes }
    $authoritativeRoot = Join-Path $taskTestRoot 'authoritative'
    [IO.Directory]::CreateDirectory($authoritativeRoot) | Out-Null
    $package = [pscustomobject]@{ archiveSha256 = (Get-Sha256 $script:OfficialBytes); release = [pscustomobject]@{ version = '0.2.2'; tag = '0.2.2'; assetUrl = 'https://github.com/ValdikSS/GoodbyeDPI/releases/download/0.2.2/goodbyedpi-0.2.2.zip'; assetSize = $script:OfficialBytes.Length }; files = @([pscustomobject]@{ relativePath = 'evil.exe'; sha256 = ('a' * 64) }); directory = 'untrusted-local-source' }
    $runtime = Get-AuthoritativeRuntime $authoritativeRoot $package
    Assert-NetworkTest ([IO.File]::ReadAllText($runtime.executable) -eq 'MZnetwork-test-only') 'Execution bytes did not come from authoritative ZIP.'
    Assert-NetworkTest ($script:OfficialFetchCount -eq 1) 'Initial authoritative fetch not performed.'
    $runtimeAgain = Get-AuthoritativeRuntime $authoritativeRoot $package
    Assert-NetworkTest ($script:OfficialFetchCount -eq 1) 'Protected proof was not reused offline.'
    [IO.File]::WriteAllText($runtime.executable,'MZtampered-runtime')
    Assert-NetworkThrows { Get-AuthoritativeRuntime $authoritativeRoot $package } 'Tampered protected executable accepted.'
    $package.archiveSha256 = ('0' * 64)
    Assert-NetworkThrows { Get-AuthoritativeRuntime $authoritativeRoot $package } 'Forged mutable archive digest accepted.'
    $package.release.assetUrl = 'https://github.com/Other/GoodbyeDPI/releases/download/0.2.2/goodbyedpi-0.2.2.zip'
    $beforeFetch = $script:OfficialFetchCount
    Assert-NetworkThrows { Get-AuthoritativeRuntime $authoritativeRoot $package } 'Unofficial elevated package URL accepted.'
    Assert-NetworkTest ($script:OfficialFetchCount -eq $beforeFetch) 'Unofficial source reached downloader.'

    # Fake adapter exercises orchestration and exact per-family rollback.
    $script:TestAdapter = [pscustomobject]@{ InterfaceGuid = [guid]'11111111-2222-3333-4444-555555555555'; ifIndex = 77; Name = 'offline adapter'; Status = 'Up' }
    $script:TestFamilies = @{}
    $script:TestOriginal = @{
        IPv4 = [pscustomobject]@{ family = 'IPv4'; available = $true; automatic = $true; servers = @('192.0.2.1'); staticServers = @() }
        IPv6 = [pscustomobject]@{ family = 'IPv6'; available = $true; automatic = $false; servers = @('2001:db8::53'); staticServers = @('2001:db8::53') }
    }
    function Reset-TestFamilies {
        foreach ($key in @('IPv4','IPv6')) { $script:TestFamilies[$key] = $script:TestOriginal[$key] | ConvertTo-Json | ConvertFrom-Json }
        $script:TestSetCalls = @()
        $script:TestDohCalls = 0
        $script:FailIpv6Once = $false
    }
    function Get-AdapterByGuid([string] $Guid) {
        if ([guid]$Guid -ne $script:TestAdapter.InterfaceGuid) { throw 'Different stable interface ID.' }
        return $script:TestAdapter
    }
    function Get-FamilyState($Adapter, [string] $Family) { return $script:TestFamilies[$Family] }
    function Set-FamilyState($Adapter, $State) {
        if ($script:FailIpv6Once -and $State.family -eq 'IPv6') { $script:FailIpv6Once = $false; throw 'Injected IPv6 failure.' }
        $script:TestSetCalls += ($State | ConvertTo-Json -Compress)
        $script:TestFamilies[$State.family] = $State | ConvertTo-Json | ConvertFrom-Json
    }
    $script:FakeDohEntries = @($script:CloudflareAddresses | ForEach-Object { [pscustomobject]@{ address = $_; existed = $true; template = 'https://previous.test/dns-query'; autoUpgrade = $false; fallback = $true } })
    function Get-CloudflareDohState { $entries = @(); foreach ($entry in $script:FakeDohEntries) { $entries += ($entry | ConvertTo-Json | ConvertFrom-Json) }; return [pscustomobject]@{ supported = $true; entries = $entries } }
    function Set-CloudflareDoh { $script:TestDohCalls++; $script:FakeDohEntries = @($script:CloudflareAddresses | ForEach-Object { [pscustomobject]@{ address = $_; existed = $true; template = $script:CloudflareTemplate; autoUpgrade = $true; fallback = $false } }) }
    function Restore-CloudflareDoh($State, [bool] $CheckOwnership) { $script:TestDohCalls++; $script:FakeDohEntries = @(); foreach ($entry in $State.entries) { $script:FakeDohEntries += ($entry | ConvertTo-Json | ConvertFrom-Json) } }
    $fakeRoot = Join-Path $taskTestRoot 'fake-protected'
    [IO.Directory]::CreateDirectory($fakeRoot) | Out-Null
    $parameters = [pscustomobject]@{ interfaceGuid = $script:TestAdapter.InterfaceGuid.ToString('D'); encrypted = $false }
    Reset-TestFamilies
    Enable-CloudflareDns $fakeRoot $parameters | Out-Null
    Assert-NetworkTest ($script:TestFamilies.IPv4.staticServers[0] -eq '1.1.1.1') 'IPv4 Cloudflare DNS was not selected.'
    Assert-NetworkTest ($script:TestFamilies.IPv6.staticServers[0] -eq '2606:4700:4700::1111') 'IPv6 Cloudflare DNS was not selected.'
    Assert-NetworkThrows { Enable-CloudflareDns $fakeRoot $parameters } 'Repeated enable overwrote original rollback.'
    Restore-CloudflareDns $fakeRoot | Out-Null
    Assert-NetworkTest $script:TestFamilies.IPv4.automatic 'IPv4 DHCP source not restored.'
    Assert-NetworkTest (-not $script:TestFamilies.IPv6.automatic) 'IPv6 static source not restored.'
    Assert-NetworkTest ($script:TestFamilies.IPv6.staticServers[0] -eq '2001:db8::53') 'Original IPv6 server not restored.'
    Assert-NetworkTest ($script:TestDohCalls -eq 0) 'Plain DNS operation unexpectedly changed DoH definitions.'
    Assert-NetworkTest (-not (Test-Path -LiteralPath (Join-Path $fakeRoot 'dns-rollback.json'))) 'Completed rollback record not removed.'

    Reset-TestFamilies
    $script:FailIpv6Once = $true
    Assert-NetworkThrows { Enable-CloudflareDns $fakeRoot $parameters } 'IPv6 failure was not reported.'
    Assert-NetworkTest $script:TestFamilies.IPv4.automatic 'Failed enable did not roll IPv4 back to DHCP.'
    Assert-NetworkTest ($script:TestFamilies.IPv6.staticServers[0] -eq '2001:db8::53') 'Failed enable did not restore IPv6.'

    Reset-TestFamilies
    Enable-CloudflareDns $fakeRoot $parameters | Out-Null
    $script:TestFamilies.IPv4.staticServers = @('203.0.113.53')
    $before = $script:TestSetCalls.Count
    Assert-NetworkThrows { Restore-CloudflareDns $fakeRoot } 'Externally changed DNS was overwritten.'
    Assert-NetworkTest ($script:TestSetCalls.Count -eq $before) 'Ownership failure changed an address family.'
    Assert-NetworkTest (Test-Path -LiteralPath (Join-Path $fakeRoot 'dns-rollback.json')) 'Ownership conflict discarded recovery snapshot.'
    # Explicitly restore our fake state for the next test; these are memory values.
    $script:TestFamilies.IPv4.staticServers = @('1.1.1.1','1.0.0.1')
    $script:FailIpv6Once = $true
    Assert-NetworkThrows { Restore-CloudflareDns $fakeRoot } 'Partial restore failure not reported.'
    $record = Read-Json (Join-Path $fakeRoot 'dns-rollback.json')
    Assert-NetworkTest (-not $record.applied) 'Partial recovery phase was not persisted.'
    Restore-CloudflareDns $fakeRoot | Out-Null
    Assert-NetworkTest $script:TestFamilies.IPv4.automatic 'Recovery retry did not restore DHCP.'
    Assert-NetworkTest ($script:TestFamilies.IPv6.staticServers[0] -eq '2001:db8::53') 'Recovery retry did not restore static IPv6.'

    # Drift after a failed/partial operation still must not be overwritten.
    Reset-TestFamilies
    Enable-CloudflareDns $fakeRoot $parameters | Out-Null
    $record = Read-Json (Join-Path $fakeRoot 'dns-rollback.json')
    $record.applied = $false
    Write-Json (Join-Path $fakeRoot 'dns-rollback.json') $record
    $script:TestFamilies.IPv4.staticServers = @('203.0.113.53')
    $before = $script:TestSetCalls.Count
    Assert-NetworkThrows { Restore-CloudflareDns $fakeRoot } 'Partial recovery overwrote subsequent external DNS change.'
    Assert-NetworkTest ($script:TestSetCalls.Count -eq $before) 'Partial DNS conflict performed a write.'
    $script:TestFamilies.IPv4.staticServers = @('1.1.1.1','1.0.0.1')
    Restore-CloudflareDns $fakeRoot | Out-Null
    Reset-TestFamilies
    $parameters.encrypted = $true
    Enable-CloudflareDns $fakeRoot $parameters | Out-Null
    $script:FakeDohEntries[0].fallback = $true
    $before = $script:TestSetCalls.Count
    Assert-NetworkThrows { Restore-CloudflareDns $fakeRoot } 'Changed DoH policy was overwritten.'
    Assert-NetworkTest ($script:TestSetCalls.Count -eq $before) 'DoH conflict performed an address-family write.'
    $record = Read-Json (Join-Path $fakeRoot 'dns-rollback.json')
    $record.applied = $false
    Write-Json (Join-Path $fakeRoot 'dns-rollback.json') $record
    Assert-NetworkThrows { Restore-CloudflareDns $fakeRoot } 'Partial recovery bypassed DoH drift.'
    $script:FakeDohEntries[0].fallback = $false
    Restore-CloudflareDns $fakeRoot | Out-Null
    Assert-NetworkTest ($script:FakeDohEntries[0].template -eq 'https://previous.test/dns-query') 'Original DoH template not restored.'

    # Process objects and kill methods are entirely mocked. No real process is
    # opened or terminated: parent/child PID reuse and ownership are simulated.
    $script:FakeProcesses = @{}
    $script:KilledCount = 0
    $script:PinCount = 0
    $script:DisposeCount = 0
    function New-FakeProcess([int] $Id, [string] $Path, [DateTime] $Start) {
        $process = [pscustomobject]@{ Id = $Id; Path = $Path; StartTime = $Start; HasExited = $false }
        $process | Add-Member -MemberType ScriptProperty -Name Handle -Value { $script:PinCount++; return 42 }
        $process | Add-Member -MemberType ScriptMethod -Name Dispose -Value { $script:DisposeCount++ }
        $process | Add-Member -MemberType ScriptMethod -Name Kill -Value { $script:KilledCount++; $this.HasExited = $true }
        $process | Add-Member -MemberType ScriptMethod -Name WaitForExit -Value { param($Duration) return $true }
        return $process
    }
    function Get-Process([int] $Id) { return $script:FakeProcesses[$Id] }
    function Get-CimInstance($ClassName,$Filter) { return [pscustomobject]@{ test = $true } }
    $script:FakeOwnerSid = $script:CallerSid
    function Invoke-CimMethod($InputObject,$MethodName) { return [pscustomobject]@{ ReturnValue = 0; Sid = $script:FakeOwnerSid } }
    $start = [DateTime]::UtcNow
    $child = New-FakeProcess 222 'F:\test\goodbyedpi.exe' $start
    $parent = New-FakeProcess 111 'F:\test\game-library-native.exe' $start
    $script:FakeProcesses[222] = $child
    $script:FakeProcesses[111] = $parent
    $state = [pscustomobject]@{ pid = 222; executable = $child.Path; startTicks = $start.Ticks.ToString(); ownerSid = $script:CallerSid }
    Assert-NetworkTest ([bool](Get-OwnedProcess $state)) 'Owned child identity rejected.'
    $state.startTicks = ($start.Ticks + 1).ToString()
    Assert-NetworkTest (-not (Get-OwnedProcess $state)) 'Recycled PID/start time accepted.'
    $state.startTicks = $start.Ticks.ToString()
    $script:FakeOwnerSid = 'S-1-5-21-1-2-3-999'
    Assert-NetworkTest (-not (Get-OwnedProcess $state)) 'Wrong process owner accepted.'
    $script:FakeOwnerSid = $script:CallerSid
    function Start-Sleep([int] $Seconds) { $script:FakeProcesses[111].HasExited = $true }
    $guardData = [pscustomobject]@{ pid = 222; appPid = 111; executable = $child.Path; childTicks = $start.Ticks.ToString(); appTicks = $start.Ticks.ToString() }
    function Run-FakeGuardian($Data) {
        $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($Data | ConvertTo-Json -Compress)))
        & ([scriptblock]::Create((New-GuardianScript $encoded)))
    }
    $beforePins = $script:PinCount
    Run-FakeGuardian $guardData
    Assert-NetworkTest ($script:KilledCount -eq 1) 'Matching child was not stopped after parent exit.'
    Assert-NetworkTest ($script:PinCount -ge $beforePins + 2) 'Guardian did not pin both identities.'
    $child.HasExited = $false
    $parent.HasExited = $false
    $guardData.childTicks = ($start.Ticks + 1).ToString()
    Run-FakeGuardian $guardData
    Assert-NetworkTest ($script:KilledCount -eq 1) 'Guardian killed a recycled child PID.'
    $guardData.childTicks = $start.Ticks.ToString()
    $guardData.appTicks = ($start.Ticks + 1).ToString()
    Run-FakeGuardian $guardData
    Assert-NetworkTest ($script:KilledCount -eq 2) 'Recycled parent PID kept the owned child alive.'
    Assert-NetworkTest ($script:DisposeCount -ge 6) 'Guardian/process handles were not released.'
    Write-Output ("PASS: $script:NetworkTestCount offline network assertions; no live network changes.")
} finally {
    $absolute = [IO.Path]::GetFullPath($taskTestRoot)
    $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $absolute.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($absolute) -notlike 'game-library-network-tests-*') { throw 'Refusing cleanup outside the verified test directory.' }
    Assert-NoReparse $absolute
    Remove-Item -LiteralPath $absolute -Recurse -Force
}
