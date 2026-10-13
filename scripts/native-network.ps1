param([string] $RequestBase64, [switch] $LibraryOnly)

# Embedded into the native binary. The bootstrap hashes these exact bytes before
# creating a ScriptBlock, so elevated operations never execute a mutable .ps1 file.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$script:CloudflareAddresses = @('1.1.1.1', '1.0.0.1', '2606:4700:4700::1111', '2606:4700:4700::1001')
$script:CloudflareTemplate = 'https://cloudflare-dns.com/dns-query'

function Assert-NoReparse([string] $Path) {
    $candidate = [IO.Path]::GetFullPath($Path)
    while ($candidate) {
        if (Test-Path -LiteralPath $candidate) {
            $item = Get-Item -LiteralPath $candidate -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Bağlantı/junction içeren ağ aracı yolu kabul edilmedi.' }
        }
        $parent = [IO.Path]::GetDirectoryName($candidate)
        if ($parent -eq $candidate) { break }
        $candidate = $parent
    }
}

function Assert-ChildPath([string] $Root, [string] $Path) {
    $rootPath = [IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
    $fullPath = [IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith($rootPath, [StringComparison]::OrdinalIgnoreCase)) { throw 'Ağ aracı yolu beklenen klasörün dışında.' }
    Assert-NoReparse $fullPath
    return $fullPath
}

function Write-Json([string] $Path, $Value) {
    Assert-NoReparse $Path
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Value | ConvertTo-Json -Depth 16 -Compress))
    $temporary = $Path + '.next'
    Assert-NoReparse $temporary
    [IO.File]::WriteAllBytes($temporary, $bytes)
    if ([IO.File]::Exists($Path)) {
        $backup = $Path + '.previous-' + [guid]::NewGuid().ToString('N')
        Assert-NoReparse $backup
        [IO.File]::Replace($temporary, $Path, $backup)
        Remove-Item -LiteralPath $backup -ErrorAction SilentlyContinue
    }
    else { [IO.File]::Move($temporary, $Path) }
}

function Read-Json([string] $Path) {
    Assert-NoReparse $Path
    if ((Get-Item -LiteralPath $Path).Length -gt 262144) { throw 'Ağ aracı kayıt dosyası çok büyük.' }
    return ([IO.File]::ReadAllText($Path) | ConvertFrom-Json)
}

function Read-ProtectedJson([string] $Path) {
    Assert-ProtectedAcl $Path | Out-Null
    return (Read-Json $Path)
}

function Get-Sha256([byte[]] $Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($Bytes)).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Get-ProtectedRoot([string] $DataDirectory, [string] $UserSid) {
    if ($UserSid -notmatch '^S-1-(5|12)-[0-9]+(?:-[0-9]+){1,8}$') { throw 'Kullanıcı kimliği doğrulanamadı.' }
    [Security.Principal.SecurityIdentifier]::new($UserSid) | Out-Null
    $identity = [Text.Encoding]::UTF8.GetBytes(([IO.Path]::GetFullPath($DataDirectory).TrimEnd('\').ToLowerInvariant()))
    $key = (Get-Sha256 $identity).Substring(0, 24)
    return (Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) ('GameLibraryNetwork\' + $UserSid + '\' + $key))
}

function Test-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    return [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function New-ProtectedDirectoryAcl([string] $ReaderSid) {
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $admin = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
    $inherit = [Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
    foreach ($sid in @('S-1-5-32-544','S-1-5-18')) {
        $principal = [Security.Principal.SecurityIdentifier]::new($sid)
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($principal, 'FullControl', $inherit, 'None', 'Allow'))
    }
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($ReaderSid), 'ReadAndExecute', $inherit, 'None', 'Allow'))
    $acl.SetOwner($admin)
    return $acl
}

function Assert-ProtectedAcl([string] $Path) {
    Assert-NoReparse $Path
    $acl = Get-Acl -LiteralPath $Path
    $owner = $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -notin @('S-1-5-32-544','S-1-5-18')) { throw 'Korunan ağ aracı kaydının sahibi yönetici değil.' }
    $writeRights = [Security.AccessControl.FileSystemRights]'Write, Delete, DeleteSubdirectoriesAndFiles, ChangePermissions, TakeOwnership'
    foreach ($rule in @($acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))) {
        if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin @('S-1-5-32-544','S-1-5-18') -and ($rule.FileSystemRights -band $writeRights) -ne 0) { throw 'Korunan ağ aracı kaydı yönetici dışında yazılabilir.' }
    }
    return $acl
}

function New-ProtectedDirectory([string] $Path, [string] $ReaderSid) {
    Assert-NoReparse $Path
    if (-not (Test-Path -LiteralPath $Path)) {
        # .NET Framework creates with this ACL/owner atomically. A later Set-Acl
        # would leave a window in which a creator-owned object can be replaced.
        [IO.DirectoryInfo]::new($Path).Create((New-ProtectedDirectoryAcl $ReaderSid))
    }
    Assert-ProtectedAcl $Path | Out-Null
}

function Write-ProtectedBytes([string] $Path, [byte[]] $Bytes) {
    $parent = Assert-ProtectedAcl ([IO.Path]::GetDirectoryName($Path))
    Assert-NoReparse $Path
    if (Test-Path -LiteralPath $Path) { Assert-ProtectedAcl $Path | Out-Null }
    $acl = [Security.AccessControl.FileSecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
    foreach ($rule in @($parent.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($rule.IdentityReference, $rule.FileSystemRights, $rule.AccessControlType))
    }
    $temporary = $Path + '.write-' + [guid]::NewGuid().ToString('N')
    $stream = [IO.FileStream]::new($temporary,[IO.FileMode]::CreateNew,[Security.AccessControl.FileSystemRights]::Write,[IO.FileShare]::None,4096,[IO.FileOptions]::None,$acl)
    try { $stream.Write($Bytes,0,$Bytes.Length); $stream.Flush($true) }
    finally { $stream.Dispose() }
    try {
        if ([IO.File]::Exists($Path)) {
            $backup = $Path + '.previous-' + [guid]::NewGuid().ToString('N')
            [IO.File]::Replace($temporary,$Path,$backup)
            Remove-Item -LiteralPath $backup -ErrorAction SilentlyContinue
        } else { [IO.File]::Move($temporary,$Path) }
        Assert-ProtectedAcl $Path | Out-Null
    } finally { if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary } }
}

function Write-ProtectedJson([string] $Path, $Value) {
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Value | ConvertTo-Json -Depth 16 -Compress))
    Write-ProtectedBytes $Path $bytes
}

function Initialize-ProtectedRoot([string] $Root, [string] $UserSid) {
    if (-not (Test-Administrator)) { throw 'Bu işlem için Windows yönetici izni gerekir.' }
    $base = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'GameLibraryNetwork'
    $directories = @($base, (Join-Path $base $UserSid), $Root)
    foreach ($item in $directories) {
        $reader = if ($item -eq $base) { 'S-1-5-32-545' } else { $UserSid }
        New-ProtectedDirectory $item $reader
    }
}

function Get-AdapterByGuid([string] $Guid) {
    $normalized = ([guid]::Parse($Guid)).ToString('D')
    $adapter = @(Get-NetAdapter -IncludeHidden | Where-Object { ([guid] $_.InterfaceGuid).ToString('D') -eq $normalized })
    if ($adapter.Count -ne 1) { throw 'Kaydedilen ağ bağdaştırıcısı bulunamadı; başka bağdaştırıcı değiştirilmedi.' }
    return $adapter[0]
}

function Get-FamilyState($Adapter, [string] $Family) {
    $service = if ($Family -eq 'IPv4') { 'Tcpip' } else { 'Tcpip6' }
    $registry = 'HKLM:\SYSTEM\CurrentControlSet\Services\' + $service + '\Parameters\Interfaces\' + ([guid] $Adapter.InterfaceGuid).ToString('B')
    $raw = ''
    if (Test-Path -LiteralPath $registry) {
        $properties = Get-ItemProperty -LiteralPath $registry
        if ($properties.PSObject.Properties['NameServer']) { $raw = [string]::Join(',', @($properties.NameServer)) }
    }
    $static = @($raw -split '[,;\s]+' | Where-Object { $_ })
    $current = @(Get-DnsClientServerAddress -InterfaceIndex $Adapter.ifIndex -AddressFamily $Family -ErrorAction SilentlyContinue)
    if ($current.Count -gt 1) { throw ('DNS adres ailesi tekil değil: ' + $Family) }
    return [pscustomobject]@{ family = $Family; available = ($current.Count -eq 1); automatic = ($static.Count -eq 0); servers = @($current | ForEach-Object { $_.ServerAddresses }); staticServers = $static }
}

function Set-FamilyState($Adapter, $State) {
    if ($State.family -notin @('IPv4', 'IPv6')) { throw 'DNS adres ailesi geçersiz.' }
    if ($State.PSObject.Properties['available'] -and -not $State.available) { return }
    # Re-resolve before every write: an interface index may be reused after an
    # adapter is removed while the UAC/helper operation is in progress.
    $Adapter = Get-AdapterByGuid ([string]$Adapter.InterfaceGuid)
    $current = @(Get-DnsClientServerAddress -InterfaceIndex $Adapter.ifIndex -AddressFamily $State.family)
    if ($current.Count -ne 1) { throw 'DNS bağdaştırıcısı tekrar doğrulanamadı.' }
    if ([bool] $State.automatic) { Set-DnsClientServerAddress -InputObject $current[0] -ResetServerAddresses }
    else {
        $servers = @($State.staticServers)
        if ($servers.Count -eq 0 -or $servers.Count -gt 12) { throw 'Kaydedilen statik DNS listesi geçersiz.' }
        foreach ($server in $servers) {
            $ip = [Net.IPAddress]::Parse($server)
            $family = if ($ip.AddressFamily -eq [Net.Sockets.AddressFamily]::InterNetwork) { 'IPv4' } else { 'IPv6' }
            if ($family -ne $State.family) { throw 'DNS kaydının adres ailesi uyuşmuyor.' }
        }
        Set-DnsClientServerAddress -InputObject $current[0] -ServerAddresses $servers
    }
}

function Get-CloudflareDohState {
    $supported = [bool](Get-Command Get-DnsClientDohServerAddress -ErrorAction SilentlyContinue)
    $entries = @()
    if ($supported) {
        $all = @(Get-DnsClientDohServerAddress)
        foreach ($address in $script:CloudflareAddresses) {
            $entry = @($all | Where-Object { $_.ServerAddress -eq $address })
            if ($entry.Count -gt 1) { throw 'DoH tanımı tekil değil.' }
            if ($entry.Count -eq 0) { $entries += [pscustomobject]@{ address = $address; existed = $false } }
            else { $entries += [pscustomobject]@{ address = $address; existed = $true; template = $entry[0].DohTemplate; fallback = [bool]$entry[0].AllowFallbackToUdp; autoUpgrade = [bool]$entry[0].AutoUpgrade } }
        }
    }
    return [pscustomobject]@{ supported = $supported; entries = $entries }
}

function Set-CloudflareDoh {
    foreach ($command in @('Get-DnsClientDohServerAddress', 'Set-DnsClientDohServerAddress', 'Add-DnsClientDohServerAddress', 'Remove-DnsClientDohServerAddress')) {
        if (-not (Get-Command $command -ErrorAction SilentlyContinue)) { throw 'Bu Windows sürümü yerleşik DoH desteklemiyor. Şifresiz Cloudflare DNS seçeneğini kullanabilirsin.' }
    }
    $existing = @(Get-DnsClientDohServerAddress)
    foreach ($address in $script:CloudflareAddresses) {
        $params = @{ ServerAddress = $address; DohTemplate = $script:CloudflareTemplate; AutoUpgrade = $true; AllowFallbackToUdp = $false }
        if (@($existing | Where-Object { $_.ServerAddress -eq $address }).Count -gt 0) { Set-DnsClientDohServerAddress @params }
        else { Add-DnsClientDohServerAddress @params }
    }
}

function Restore-CloudflareDoh($State, [bool] $CheckOwnership) {
    if (-not $State.supported) { return }
    $current = @(Get-DnsClientDohServerAddress)
    foreach ($entry in @($State.entries)) {
        if ($entry.address -notin $script:CloudflareAddresses) { throw 'DoH kaydının adresi geçersiz.' }
        $item = @($current | Where-Object { $_.ServerAddress -eq $entry.address })
        if ($CheckOwnership -and ($item.Count -ne 1 -or $item[0].DohTemplate -ne $script:CloudflareTemplate -or -not $item[0].AutoUpgrade -or $item[0].AllowFallbackToUdp)) {
            throw 'Cloudflare DoH ayarı başka bir uygulama tarafından değişmiş; üzerine yazılmadı.'
        }
    }
    foreach ($entry in @($State.entries)) {
        if ($entry.existed) {
            $params = @{ ServerAddress = $entry.address; DohTemplate = $entry.template; AutoUpgrade = [bool]$entry.autoUpgrade; AllowFallbackToUdp = [bool]$entry.fallback }
            if (@($current | Where-Object { $_.ServerAddress -eq $entry.address }).Count -gt 0) { Set-DnsClientDohServerAddress @params }
            else { Add-DnsClientDohServerAddress @params }
        } elseif (@($current | Where-Object { $_.ServerAddress -eq $entry.address }).Count -gt 0) {
            Remove-DnsClientDohServerAddress -ServerAddress $entry.address -Confirm:$false
        }
    }
}

function Assert-DnsOwnership($Snapshot, $Adapter) {
    foreach ($state in @($Snapshot.families)) {
        if ($state.PSObject.Properties['available'] -and -not $state.available) { continue }
        $current = Get-FamilyState $Adapter $state.family
        $expected = if ($state.family -eq 'IPv4') { $script:CloudflareAddresses[0..1] } else { $script:CloudflareAddresses[2..3] }
        if ($current.automatic -or (@($current.staticServers) -join ',') -ne ($expected -join ',')) {
            throw 'DNS ayarı başka bir uygulama tarafından değişmiş. Önceki kayıt korunuyor; yeni ayarın üzerine yazılmadı.'
        }
    }
}

function Compare-DnsAddresses($First, $Second) {
    $left = @($First | ForEach-Object { ([Net.IPAddress]::Parse([string]$_)).ToString() })
    $right = @($Second | ForEach-Object { ([Net.IPAddress]::Parse([string]$_)).ToString() })
    return (($left -join ',') -eq ($right -join ','))
}

function Assert-RecoveryOwnership($Snapshot, $Adapter) {
    foreach ($original in @($Snapshot.families)) {
        if ($original.PSObject.Properties['available'] -and -not $original.available) { continue }
        $current = Get-FamilyState $Adapter $original.family
        $expected = if ($original.family -eq 'IPv4') { $script:CloudflareAddresses[0..1] } else { $script:CloudflareAddresses[2..3] }
        $ours = -not $current.automatic -and (Compare-DnsAddresses $current.staticServers $expected)
        $restored = if ($original.automatic) { [bool]$current.automatic } else { -not $current.automatic -and (Compare-DnsAddresses $current.staticServers $original.staticServers) }
        if (-not $ours -and ($Snapshot.applied -or -not $restored)) { throw 'DNS ayarı geri alma kaydı dışında değiştirilmiş; üzerine yazılmadı.' }
    }
    if ($Snapshot.encrypted -and $Snapshot.doh.supported) {
        $current = Get-CloudflareDohState
        if (-not $current.supported -or @($current.entries).Count -ne 4) { throw 'DoH ayarları doğrulanamadı; DNS değiştirilmedi.' }
        foreach ($original in @($Snapshot.doh.entries)) {
            $item = @($current.entries | Where-Object { $_.address -eq $original.address })
            if ($item.Count -ne 1) { throw 'DoH geri alma adresi doğrulanamadı.' }
            $ours = $item[0].existed -and $item[0].template -eq $script:CloudflareTemplate -and $item[0].autoUpgrade -and -not $item[0].fallback
            $restored = if ($original.existed) { $item[0].existed -and $item[0].template -eq $original.template -and $item[0].autoUpgrade -eq $original.autoUpgrade -and $item[0].fallback -eq $original.fallback } else { -not $item[0].existed }
            if (-not $ours -and ($Snapshot.applied -or -not $restored)) { throw 'DoH ayarı geri alma kaydı dışında değiştirilmiş; üzerine yazılmadı.' }
        }
    }
}

function Enable-CloudflareDns([string] $Root, $Params) {
    $snapshotPath = Join-Path $Root 'dns-rollback.json'
    if (Test-Path -LiteralPath $snapshotPath) { throw 'Önce mevcut Cloudflare DNS oturumunu geri yükle.' }
    $adapter = Get-AdapterByGuid $Params.interfaceGuid
    if ($adapter.Status -ne 'Up') { throw 'Etkin bir ağ bağdaştırıcısı seç.' }
    $doh = Get-CloudflareDohState
    if ($Params.encrypted -and -not $doh.supported) { throw 'Bu Windows sürümünde yerleşik DoH bulunamadı.' }
    $snapshot = [pscustomobject]@{ schema = 1; interfaceGuid = ([guid] $adapter.InterfaceGuid).ToString('D'); name = $adapter.Name; families = @((Get-FamilyState $adapter 'IPv4'), (Get-FamilyState $adapter 'IPv6')); doh = $doh; encrypted = [bool]$Params.encrypted; applied = $false; timestamp = [DateTime]::UtcNow.ToString('o') }
    if (@($snapshot.families | Where-Object { $_.available }).Count -eq 0) { throw 'Bağdaştırıcının DNS ayarları okunamadı; değişiklik yapılmadı.' }
    Write-ProtectedJson $snapshotPath $snapshot
    try {
        if ($Params.encrypted) { Set-CloudflareDoh }
        foreach ($familyState in @($snapshot.families | Where-Object { $_.available })) {
            $family = $familyState.family
            $servers = if ($family -eq 'IPv4') { $script:CloudflareAddresses[0..1] } else { $script:CloudflareAddresses[2..3] }
            Set-FamilyState $adapter ([pscustomobject]@{ family = $family; automatic = $false; staticServers = $servers })
        }
        Assert-DnsOwnership $snapshot $adapter
        $snapshot.applied = $true
        Write-ProtectedJson $snapshotPath $snapshot
        return [pscustomobject]@{ dnsEnabled = $true; encrypted = [bool]$Params.encrypted; interfaceGuid = $snapshot.interfaceGuid; message = 'Cloudflare DNS uygulandı; önceki IPv4/IPv6 ve DoH ayarları kaydedildi.' }
    } catch {
        $failure = $_.Exception.Message
        try {
            Restore-CloudflareDns $Root | Out-Null
        } catch { throw ($failure + ' Geri alma tamamlanamadı; korunan geri alma kaydı saklandı: ' + $_.Exception.Message) }
        throw ($failure + ' Önceki ayarlar geri yüklendi.')
    }
}

function Restore-CloudflareDns([string] $Root) {
    $snapshotPath = Join-Path $Root 'dns-rollback.json'
    if (-not (Test-Path -LiteralPath $snapshotPath)) { return [pscustomobject]@{ dnsEnabled = $false; message = 'Uygulamaya ait DNS geri alma kaydı yok.' } }
    $snapshot = Read-ProtectedJson $snapshotPath
    if ($snapshot.schema -ne 1) { throw 'DNS geri alma kaydı sürümü geçersiz.' }
    $adapter = Get-AdapterByGuid $snapshot.interfaceGuid
    # In a partial recovery, each value must still equal either our applied
    # value or its already-restored original. A failed retry never grants
    # permission to overwrite unrelated changes made afterwards.
    Assert-RecoveryOwnership $snapshot $adapter
    # Persist the recovery phase before the first change. If a later family/DoH
    # restore fails, the exact original snapshot remains available for retry.
    $snapshot.applied = $false
    Write-ProtectedJson $snapshotPath $snapshot
    foreach ($family in @($snapshot.families)) { Set-FamilyState $adapter $family }
    if ($snapshot.encrypted) { Restore-CloudflareDoh $snapshot.doh $false }
    Remove-Item -LiteralPath $snapshotPath
    return [pscustomobject]@{ dnsEnabled = $false; interfaceGuid = $snapshot.interfaceGuid; message = 'Önceki DNS adresleri, otomatik/statik seçimi ve DoH tanımları geri yüklendi.' }
}

function Test-SafeZipName([string] $Name) {
    if (-not $Name -or $Name.Length -gt 240 -or $Name.Contains('\') -or $Name.Contains(':') -or $Name.StartsWith('/')) { return $false }
    $segments = $Name.TrimEnd('/').Split('/')
    foreach ($segment in $segments) {
        if (-not $segment -or $segment -in @('.', '..') -or $segment -notmatch '^[a-zA-Z0-9_.-]+$' -or $segment.EndsWith('.') -or $segment.EndsWith(' ')) { return $false }
        if ($segment -match '^(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)') { return $false }
    }
    return $true
}

function Expand-SafePackage([string] $Archive, [string] $Target) {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    Assert-NoReparse $Archive
    Assert-NoReparse $Target
    if ((Get-Item -LiteralPath $Archive).Length -gt 33554432) { throw 'ZIP 32 MiB sınırını aşıyor.' }
    if (Test-Path -LiteralPath $Target) { throw 'Paket hedefi zaten mevcut.' }
    $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        if ($zip.Entries.Count -eq 0 -or $zip.Entries.Count -gt 512) { throw 'ZIP dosya sayısı geçersiz.' }
        $total = [long]0
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $zip.Entries) {
            if (-not (Test-SafeZipName $entry.FullName) -or -not $names.Add($entry.FullName.TrimEnd('/'))) { throw 'ZIP yol adı veya yinelenen dosya geçersiz.' }
            $unixMode = ($entry.ExternalAttributes -shr 16) -band 0xF000
            if ($unixMode -eq 0xA000) { throw 'ZIP sembolik bağlantı içeriyor.' }
            $total += $entry.Length
            if ($entry.Length -gt 33554432 -or $total -gt 67108864) { throw 'ZIP açılmış boyut sınırını aşıyor.' }
            if ($entry.Length -gt 1048576 -and $entry.CompressedLength -gt 0 -and ($entry.Length / $entry.CompressedLength) -gt 200) { throw 'ZIP sıkıştırma oranı sınırı aşıldı.' }
        }
        New-Item -Path $Target -ItemType Directory | Out-Null
        foreach ($entry in $zip.Entries) {
            $output = Assert-ChildPath $Target (Join-Path $Target $entry.FullName)
            if ($entry.FullName.EndsWith('/')) { [IO.Directory]::CreateDirectory($output) | Out-Null; continue }
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($output)) | Out-Null
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $output, $false)
        }
    } finally { $zip.Dispose() }
}

function Prepare-Package([string] $DataDirectory, $Params) {
    if ($Params.version -notmatch '^\d+\.\d+\.\d+$' -or $Params.archiveSha256 -notmatch '^[a-f0-9]{64}$') { throw 'Paket kaydı geçersiz.' }
    $root = Join-Path $DataDirectory 'network-tools'
    $archive = Assert-ChildPath $root $Params.archive
    if ((Get-Sha256 ([IO.File]::ReadAllBytes($archive))) -ne $Params.archiveSha256) { throw 'ZIP özeti değişmiş.' }
    $packages = Join-Path $root 'packages'
    Assert-NoReparse $packages
    [IO.Directory]::CreateDirectory($packages) | Out-Null
    $target = Join-Path $packages ($Params.version + '-' + $Params.archiveSha256.Substring(0,16))
    # Unique staging keeps an existing/running version intact, even after a failed retry.
    $staging = Join-Path $packages ('staging-' + [guid]::NewGuid().ToString('N'))
    try {
        Expand-SafePackage $archive $staging
        $architecture = if ([Environment]::Is64BitOperatingSystem) { 'x86_64' } else { 'x86' }
        if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { throw 'GoodbyeDPI ARM64 paketi doğrulanmadı.' }
        $executables = @(Get-ChildItem -LiteralPath $staging -Filter 'goodbyedpi.exe' -File -Recurse | Where-Object { $_.Directory.Name -eq $architecture })
        if ($executables.Count -ne 1) { throw 'İşletim sistemi mimarisine uygun GoodbyeDPI bulunamadı.' }
        $selected = $executables[0].Directory.FullName
        $files = @()
        foreach ($name in @('goodbyedpi.exe', 'WinDivert.dll', 'WinDivert32.sys', 'WinDivert64.sys')) {
            $path = Join-Path $selected $name
            if (-not (Test-Path -LiteralPath $path)) {
                if ($name -eq 'WinDivert64.sys' -or $name -eq 'WinDivert32.sys') { continue }
                throw ('Paket bileşeni yok: ' + $name)
            }
            $bytes = [IO.File]::ReadAllBytes($path)
            if ($bytes.Length -lt 2 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw 'Paket bileşeni Windows PE dosyası değil.' }
            $relative = $path.Substring($staging.Length + 1).Replace('\','/')
            $files += [pscustomobject]@{ relativePath = $relative; size = $bytes.Length; sha256 = (Get-Sha256 $bytes) }
        }
        $driver = if ([Environment]::Is64BitOperatingSystem) { 'WinDivert64.sys' } else { 'WinDivert32.sys' }
        if (-not (Test-Path -LiteralPath (Join-Path $selected $driver))) { throw 'İşletim sistemine uygun WinDivert sürücüsü yok.' }
        $executable = $executables[0].FullName.Substring($staging.Length + 1).Replace('\','/')
        if (Test-Path -LiteralPath $target) {
            # Existing same digest package must be identical; never overwrite active files.
            Assert-NoReparse $target
            foreach ($file in $files) {
                $existing = Assert-ChildPath $target (Join-Path $target $file.relativePath)
                if (-not (Test-Path -LiteralPath $existing) -or (Get-Sha256 ([IO.File]::ReadAllBytes($existing))) -ne $file.sha256) { throw 'Mevcut paket değiştirilmiş; paketi kaldırıp yeniden hazırla.' }
            }
        } else { [IO.Directory]::Move($staging, $target) }
        return [pscustomobject]@{ directory = $target; executable = $executable; files = $files }
    } finally {
        # Recursive removal only after absolute containment/no-reparse validation.
        if (Test-Path -LiteralPath $staging) { $safe = Assert-ChildPath $packages $staging; Remove-Item -LiteralPath $safe -Recurse -Force }
    }
}

function Read-OfficialAsset([string] $Address, [long] $Limit) {
    Add-Type -AssemblyName System.Net.Http
    # Windows PowerShell/.NET Framework may otherwise default to legacy TLS.
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $initial = [uri]$Address
    $current = $initial
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds(45)
    $client.DefaultRequestHeaders.UserAgent.ParseAdd('GameLibrary-Network/0.1')
    $cancel = [Threading.CancellationTokenSource]::new(45000)
    try {
        for ($redirect = 0; $redirect -le 3; $redirect++) {
            if ($current.Scheme -ne 'https' -or $current.Port -ne 443 -or $current.UserInfo -or $current.Fragment) { throw 'Resmi paket HTTPS adresi geçersiz.' }
            $allowed = ($current.Host -eq 'github.com' -and $current.AbsolutePath -eq $initial.AbsolutePath -and -not $current.Query) -or ($current.Host -eq 'release-assets.githubusercontent.com' -and $current.AbsolutePath.StartsWith('/github-production-release-asset/')) -or ($current.Host -eq 'objects.githubusercontent.com' -and $current.AbsolutePath.StartsWith('/github-production-release-asset-'))
            if (-not $allowed) { throw 'Resmi paket başka kaynağa yönlendi.' }
            $message = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Get,$current)
            $response = $client.SendAsync($message,[Net.Http.HttpCompletionOption]::ResponseHeadersRead,$cancel.Token).GetAwaiter().GetResult()
            try {
                $code = [int]$response.StatusCode
                if ($code -ge 300 -and $code -lt 400) {
                    if (-not $response.Headers.Location) { throw 'Resmi paket yönlendirmesi eksik.' }
                    $current = [uri]::new($current,$response.Headers.Location)
                    continue
                }
                if (-not $response.IsSuccessStatusCode) { throw 'Resmi GoodbyeDPI paketi indirilemedi.' }
                if ($response.Content.Headers.ContentLength -gt $Limit) { throw 'Resmi paket boyut sınırını aşıyor.' }
                $input = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
                $output = [IO.MemoryStream]::new()
                try {
                    $buffer = [byte[]]::new(65536)
                    while (($read = $input.ReadAsync($buffer,0,$buffer.Length,$cancel.Token).GetAwaiter().GetResult()) -gt 0) {
                        if ($output.Length + $read -gt $Limit) { throw 'Resmi paket boyut sınırını aşıyor.' }
                        $output.Write($buffer,0,$read)
                    }
                    return ,$output.ToArray()
                } finally { $input.Dispose(); $output.Dispose() }
            } finally { $response.Dispose(); $message.Dispose() }
        }
        throw 'Resmi paket yönlendirme sınırını aşıyor.'
    } finally { $cancel.Dispose(); $client.Dispose(); $handler.Dispose() }
}

function Read-OfficialComponents([byte[]] $Bytes) {
    Add-Type -AssemblyName System.IO.Compression
    $memory = [IO.MemoryStream]::new($Bytes,$false)
    $zip = [IO.Compression.ZipArchive]::new($memory,[IO.Compression.ZipArchiveMode]::Read)
    try {
        if ($zip.Entries.Count -eq 0 -or $zip.Entries.Count -gt 512) { throw 'Resmi ZIP dosya sayısı geçersiz.' }
        $total = [long]0
        $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $zip.Entries) {
            if (-not (Test-SafeZipName $entry.FullName) -or -not $paths.Add($entry.FullName.TrimEnd('/'))) { throw 'Resmi ZIP dosya yolu geçersiz.' }
            $total += $entry.Length
            if ($entry.Length -gt 33554432 -or $total -gt 67108864 -or (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000) { throw 'Resmi ZIP boyutu veya bağlantısı geçersiz.' }
            if ($entry.Length -gt 1048576 -and $entry.CompressedLength -gt 0 -and ($entry.Length / $entry.CompressedLength) -gt 200) { throw 'Resmi ZIP sıkıştırma oranı geçersiz.' }
        }
        $architecture = if ([Environment]::Is64BitOperatingSystem) { 'x86_64' } else { 'x86' }
        $executables = @($zip.Entries | Where-Object { $_.FullName -match ('(^|/)' + $architecture + '/goodbyedpi\.exe$') })
        if ($executables.Count -ne 1) { throw 'Resmi paketin uygun mimaride executable dosyası bulunamadı.' }
        $prefix = $executables[0].FullName.Substring(0,$executables[0].FullName.LastIndexOf('/') + 1)
        $components = @()
        foreach ($name in @('goodbyedpi.exe','WinDivert.dll','WinDivert32.sys','WinDivert64.sys')) {
            $entry = @($zip.Entries | Where-Object { $_.FullName -eq ($prefix + $name) })
            if ($entry.Count -eq 0 -and $name -like '*.sys') { continue }
            if ($entry.Count -ne 1 -or $entry[0].Length -lt 2) { throw ('Resmi paket bileşeni eksik: ' + $name) }
            $input = $entry[0].Open()
            $output = [IO.MemoryStream]::new()
            try {
                $buffer = [byte[]]::new(65536)
                while (($read = $input.Read($buffer,0,$buffer.Length)) -gt 0) {
                    if ($output.Length + $read -gt $entry[0].Length) { throw 'Resmi ZIP bileşen boyutu uyuşmuyor.' }
                    $output.Write($buffer,0,$read)
                }
                $content = $output.ToArray()
                if ($content.Length -ne $entry[0].Length -or $content[0] -ne 0x4D -or $content[1] -ne 0x5A) { throw 'Resmi bileşen Windows PE dosyası değil.' }
                $components += [pscustomobject]@{ name = $name; size = $content.Length; sha256 = (Get-Sha256 $content); bytes = $content }
            } finally { $input.Dispose(); $output.Dispose() }
        }
        $driver = if ([Environment]::Is64BitOperatingSystem) { 'WinDivert64.sys' } else { 'WinDivert32.sys' }
        if ($driver -notin @($components.name)) { throw 'Resmi pakette uygun WinDivert sürücüsü yok.' }
        return $components
    } finally { $zip.Dispose(); $memory.Dispose() }
}

function Get-AuthoritativeRuntime([string] $Root, $Package) {
    if ($Package.archiveSha256 -notmatch '^[a-f0-9]{64}$' -or $Package.release.version -notmatch '^\d+\.\d+\.\d+$' -or $Package.release.tag -notmatch '^v?\d+\.\d+\.\d+$' -or $Package.release.tag.TrimStart('v') -ne $Package.release.version) { throw 'Resmi paket kaydı geçersiz.' }
    $expectedUrl = 'https://github.com/ValdikSS/GoodbyeDPI/releases/download/' + $Package.release.tag + '/goodbyedpi-' + $Package.release.version + '.zip'
    if ($Package.release.assetUrl -cne $expectedUrl -or $Package.release.assetSize -le 0 -or $Package.release.assetSize -gt 33554432) { throw 'Resmi paket adresi veya boyutu geçersiz.' }
    $reader = $script:CallerSid
    $runtimeRoot = Join-Path $Root 'runtime'
    New-ProtectedDirectory $runtimeRoot $reader
    $runtime = Join-Path $runtimeRoot ($Package.release.version + '-' + $Package.archiveSha256.Substring(0,16))
    New-ProtectedDirectory $runtime $reader
    $proofPath = Join-Path $runtime 'package-proof.json'
    if (Test-Path -LiteralPath $proofPath) {
        Assert-ProtectedAcl $proofPath | Out-Null
        $proof = Read-Json $proofPath
        if ($proof.schema -ne 1 -or $proof.archiveSha256 -ne $Package.archiveSha256 -or $proof.version -ne $Package.release.version -or $proof.assetUrl -cne $expectedUrl) { throw 'Korunan resmi paket kaydı uyuşmuyor.' }
    } else {
        # A LocalAppData manifest can be changed together with its file hashes.
        # Derive executable bytes from a fresh official HTTPS ZIP, never from
        # those mutable files. Later starts reuse this administrator-owned proof.
        $bytes = Read-OfficialAsset $expectedUrl ([long]$Package.release.assetSize)
        if ($bytes.Length -ne $Package.release.assetSize -or (Get-Sha256 $bytes) -ne $Package.archiveSha256) { throw 'Korunan doğrulamada resmi paket özeti uyuşmadı; paketi yeniden hazırla.' }
        $components = @(Read-OfficialComponents $bytes)
        $files = @()
        foreach ($component in $components) {
            Write-ProtectedBytes (Join-Path $runtime $component.name) $component.bytes
            $files += [pscustomobject]@{ name = $component.name; size = $component.size; sha256 = $component.sha256 }
        }
        $proof = [pscustomobject]@{ schema = 1; archiveSha256 = $Package.archiveSha256; version = $Package.release.version; assetUrl = $expectedUrl; files = $files }
        Write-ProtectedJson $proofPath $proof
    }
    $names = @()
    foreach ($file in @($proof.files)) {
        if ($file.name -notin @('goodbyedpi.exe','WinDivert.dll','WinDivert32.sys','WinDivert64.sys') -or $file.name -in $names -or $file.size -lt 2 -or $file.size -gt 33554432 -or $file.sha256 -notmatch '^[a-f0-9]{64}$') { throw 'Korunan paket bileşeni kaydı geçersiz.' }
        $names += $file.name
        $path = Join-Path $runtime $file.name
        Assert-ProtectedAcl $path | Out-Null
        if ((Get-Item -LiteralPath $path).Length -ne $file.size -or (Get-Sha256 ([IO.File]::ReadAllBytes($path))) -ne $file.sha256) { throw 'Korunan resmi paket bileşeni değiştirilmiş.' }
    }
    $driver = if ([Environment]::Is64BitOperatingSystem) { 'WinDivert64.sys' } else { 'WinDivert32.sys' }
    if ($names.Count -lt 3 -or $names.Count -gt 4 -or 'goodbyedpi.exe' -notin $names -or 'WinDivert.dll' -notin $names -or $driver -notin $names) { throw 'Korunan resmi paket bileşenleri eksik.' }
    return [pscustomobject]@{ directory = $runtime; driver = (Join-Path $runtime $driver); executable = (Join-Path $runtime 'goodbyedpi.exe') }
}

function New-GuardianScript([string] $EncodedData) {
    if ($EncodedData -notmatch '^[A-Za-z0-9+/]+={0,2}$') { throw 'Oturum gözcüsü verisi geçersiz.' }
    return @'
$ErrorActionPreference='Stop'
$d=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__DATA__'))|ConvertFrom-Json
$c=Get-Process -Id $d.pid -ErrorAction SilentlyContinue
if(-not $c){return}
$a=$null
try {
    # Pin the process handle before checking its identity. Kill() operates on
    # this object; it never reopens an unverified potentially recycled PID.
    $c.Handle|Out-Null
    if($c.StartTime.ToUniversalTime().Ticks.ToString() -ne $d.childTicks -or $c.Path -ne $d.executable){return}
    try {
        $a=Get-Process -Id $d.appPid -ErrorAction SilentlyContinue
        if($a){$a.Handle|Out-Null;if($a.StartTime.ToUniversalTime().Ticks.ToString() -ne $d.appTicks){$a.Dispose();$a=$null}}
    } catch {if($a){$a.Dispose()};$a=$null}
    while($a -and -not $a.HasExited -and -not $c.HasExited){Start-Sleep -Seconds 2}
    if(-not $c.HasExited){$c.Kill();$c.WaitForExit(5000)|Out-Null}
} finally {$c.Dispose();if($a){$a.Dispose()}}
'@.Replace('__DATA__',$EncodedData)
}

function Get-OwnedProcess($State) {
    $process = Get-Process -Id ([int]$State.pid) -ErrorAction SilentlyContinue
    if (-not $process) { return $null }
    try {
        $process.Handle | Out-Null
        if ($process.Path -ne $State.executable -or $process.StartTime.ToUniversalTime().Ticks.ToString() -ne [string]$State.startTicks) { $process.Dispose(); return $null }
        $cim = Get-CimInstance Win32_Process -Filter ('ProcessId=' + [int]$State.pid)
        $owner = Invoke-CimMethod -InputObject $cim -MethodName GetOwnerSid
        if ($owner.ReturnValue -ne 0 -or $owner.Sid -ne $State.ownerSid) { $process.Dispose(); return $null }
        return $process
    } catch { $process.Dispose(); throw ('GoodbyeDPI süreci doğrulanamadı: ' + $_.Exception.Message) }
}

function Start-GoodbyeDpi([string] $Root, [string] $DataDirectory, $Params) {
    $statePath = Join-Path $Root 'process.json'
    if (Test-Path -LiteralPath $statePath) {
        $state = Read-ProtectedJson $statePath
        $running = Get-OwnedProcess $state
        if ($running) { $running.Dispose(); return [pscustomobject]@{ running = $true; version = $state.version; pid = $state.pid; message = 'GoodbyeDPI zaten çalışıyor. Hazır güncelleme sonraki başlatmada uygulanır.' } }
    }
    if (@(Get-Process -Name goodbyedpi -ErrorAction SilentlyContinue).Count -gt 0) { throw 'Uygulama dışında bir GoodbyeDPI süreci çalışıyor; mevcut süreç değiştirilmedi.' }
    $package = $Params.package
    $authoritative = Get-AuthoritativeRuntime $Root $package
    $runtime = $authoritative.directory
    $driver = $authoritative.driver
    $signature = Get-AuthenticodeSignature -LiteralPath $driver
    if ($signature.Status -ne 'Valid') { throw ('WinDivert sürücüsünün Windows imzası doğrulanamadı: ' + $signature.Status) }
    foreach ($service in @(Get-CimInstance Win32_SystemDriver | Where-Object { $_.Name -like 'WinDivert*' })) {
        $servicePath = ([string]$service.PathName).Trim('"').Replace('\??\','')
        if (-not $servicePath.StartsWith($Root.TrimEnd('\') + '\runtime\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Başka bir uygulamaya ait WinDivert sürücüsü var; değiştirilmedi.' }
        Assert-ProtectedAcl $servicePath | Out-Null
        if ((Get-Sha256 ([IO.File]::ReadAllBytes($servicePath))) -ne (Get-Sha256 ([IO.File]::ReadAllBytes($driver)))) { throw 'Mevcut WinDivert sürücü kaydı farklı bir paket sürümüne bağlı; eski sürücüyle yeni paket başlatılmadı. Sürücü değişiminin ayrıca tamamlanması gerekir.' }
    }
    $app = Get-Process -Id ([int]$Params.appPid) -ErrorAction Stop
    if ($app.Path -ne $Params.appExecutable) { throw 'Uygulama oturumu doğrulanamadı.' }
    $executable = $authoritative.executable
    Write-ProtectedBytes (Join-Path $runtime 'output.log') ([byte[]]::new(0))
    Write-ProtectedBytes (Join-Path $runtime 'error.log') ([byte[]]::new(0))
    $process = Start-Process -FilePath $executable -WorkingDirectory $runtime -ArgumentList @('-6') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $runtime 'output.log') -RedirectStandardError (Join-Path $runtime 'error.log')
    Start-Sleep -Milliseconds 1000
    $process.Refresh()
    if ($process.HasExited) { throw ('GoodbyeDPI başlatılamadı (çıkış ' + $process.ExitCode + '). Sürücü/koruma durumunu denetle; uygulama güvenlik ayarlarını değiştirmez.') }
    try {
        $cim = Get-CimInstance Win32_Process -Filter ('ProcessId=' + $process.Id)
        $owner = Invoke-CimMethod -InputObject $cim -MethodName GetOwnerSid
        if ($owner.ReturnValue -ne 0) { throw 'Yeni sürecin sahibi okunamadı.' }
        $state = [pscustomobject]@{ pid = $process.Id; executable = $executable; startTicks = $process.StartTime.ToUniversalTime().Ticks.ToString(); ownerSid = $owner.Sid; version = $package.release.version }
        Write-ProtectedJson $statePath $state
        # A small elevated guardian owns exactly this PID/start time. It stops the
        # child when the app exits/crashes; no startup entry or persistent service.
        $guardData = [pscustomobject]@{ appPid = $app.Id; appTicks = $app.StartTime.ToUniversalTime().Ticks.ToString(); pid = $state.pid; childTicks = $state.startTicks; executable = $state.executable }
        $encodedData = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($guardData | ConvertTo-Json -Compress)))
        $guardian = New-GuardianScript $encodedData
        $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($guardian))
        $shell = Join-Path ([Environment]::GetFolderPath('System')) 'WindowsPowerShell\v1.0\powershell.exe'
        $guardProcess = Start-Process -FilePath $shell -WindowStyle Hidden -PassThru -ArgumentList @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',$encoded)
        Start-Sleep -Milliseconds 300
        $guardProcess.Refresh()
        if ($guardProcess.HasExited -and $guardProcess.ExitCode -ne 0) { throw 'GoodbyeDPI oturum gözcüsü başlatılamadı; süreç durduruldu.' }
        $process.Refresh()
        if ($process.HasExited) { throw 'Uygulama oturumu gözcü tarafından doğrulanamadı; GoodbyeDPI durduruldu.' }
        return [pscustomobject]@{ running = $true; pid = $state.pid; version = $state.version; message = 'GoodbyeDPI başlatıldı; uygulamadan çıkınca durur. Çalışan süreç bağlantının sağlandığını tek başına doğrulamaz.' }
    } catch {
        $process.Refresh()
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        throw
    }
}

function Stop-GoodbyeDpi([string] $Root) {
    $statePath = Join-Path $Root 'process.json'
    if (-not (Test-Path -LiteralPath $statePath)) { return [pscustomobject]@{ running = $false; message = 'Uygulamaya ait GoodbyeDPI oturumu yok.' } }
    $state = Read-ProtectedJson $statePath
    $process = Get-OwnedProcess $state
    if ($process) {
        try { $process.Kill(); if (-not $process.WaitForExit(5000)) { throw 'GoodbyeDPI kapanması bekleniyor; süreç kaydı yeniden denemek için korundu.' } }
        finally { $process.Dispose() }
    }
    Remove-Item -LiteralPath $statePath
    return [pscustomobject]@{ running = $false; message = 'Uygulamaya ait GoodbyeDPI durduruldu. Diğer süreçler ve servisler değiştirilmedi.' }
}

function Inspect-NetworkTools([string] $Root) {
    $interfaces = @()
    $preferred = $null
    $route = @(Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric)
    foreach ($adapter in @(Get-NetAdapter | Sort-Object ifIndex)) {
        $guid = ([guid] $adapter.InterfaceGuid).ToString('D')
        if ($adapter.Status -eq 'Up' -and $route.Count -gt 0 -and $adapter.ifIndex -eq $route[0].InterfaceIndex) { $preferred = $guid }
        $families = @((Get-FamilyState $adapter 'IPv4'), (Get-FamilyState $adapter 'IPv6'))
        $interfaces += [pscustomobject]@{ guid = $guid; name = $adapter.Name; up = ($adapter.Status -eq 'Up'); families = $families }
    }
    $dns = $null
    $dnsMatches = $false
    $dnsError = $null
    $dnsPath = Join-Path $Root 'dns-rollback.json'
    if (Test-Path -LiteralPath $dnsPath) {
        try { $dns = Read-ProtectedJson $dnsPath }
        catch { $dnsError = $_.Exception.Message }
    }
    if ($dns -and $dns.applied) {
        try { Assert-DnsOwnership $dns (Get-AdapterByGuid $dns.interfaceGuid); $dnsMatches = $true }
        catch { $dnsError = $_.Exception.Message }
    }
    elseif ($dns) { $dnsError = 'DNS işlemi yarım kaldı; özgün ayarları geri yükle.' }
    $running = $false
    $state = $null
    $ownershipError = $null
    $statePath = Join-Path $Root 'process.json'
    if (Test-Path -LiteralPath $statePath) {
        try {
            $state = Read-ProtectedJson $statePath
            $process = Get-Process -Id $state.pid -ErrorAction SilentlyContinue
            try { $running = [bool]($process -and $process.Path -eq $state.executable -and $process.StartTime.ToUniversalTime().Ticks.ToString() -eq $state.startTicks) }
            finally { if ($process) { $process.Dispose() } }
        }
        catch { $ownershipError = $_.Exception.Message }
    }
    return [pscustomobject]@{ interfaces = $interfaces; preferredInterfaceGuid = $preferred; dohSupported = [bool](Get-Command Get-DnsClientDohServerAddress -ErrorAction SilentlyContinue); dnsEnabled = $dnsMatches; rollbackAvailable = [bool]$dns; dnsError = $dnsError; dnsInterfaceGuid = $(if($dns){$dns.interfaceGuid}else{$null}); running = $running; stopAvailable = [bool]$state; runningVersion = $(if($running){$state.version}else{$null}); ownershipError = $ownershipError }
}

if ($LibraryOnly) { return }
$request = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($RequestBase64)) | ConvertFrom-Json
$response = $null
$mutation = $request.action -in @('enable-dns','restore-dns','start','stop')
try {
    $data = [IO.Path]::GetFullPath([string]$request.dataDirectory)
    Assert-NoReparse $data
    $localResponse = Assert-ChildPath (Join-Path $data 'network-tools') ([string]$request.response)
    $responseName = [IO.Path]::GetFileName($localResponse)
    if ($responseName -notmatch '^response-[a-f0-9]{32}\.json$') { throw 'Yanıt dosyası adı geçersiz.' }
    $sid = [string]$request.callerSid
    $script:CallerSid = $sid
    $root = Get-ProtectedRoot $data $sid
    if ($mutation) {
        Initialize-ProtectedRoot $root $sid
        $responses = Join-Path $root 'responses'
        New-ProtectedDirectory $responses $sid
        $response = Join-Path $responses $responseName
        # A bounded cache lets the original unelevated launcher read its result.
        # Only fixed response filenames under this protected folder are removed.
        $old = @(Get-ChildItem -LiteralPath $responses -File | Where-Object { $_.Name -match '^response-[a-f0-9]{32}\.json$' } | Sort-Object LastWriteTimeUtc -Descending)
        foreach ($file in @($old | Select-Object -Skip 63)) { Assert-ProtectedAcl $file.FullName | Out-Null; Remove-Item -LiteralPath $file.FullName }
    } else { $response = $localResponse }
    $value = switch ($request.action) {
        'inspect' { Inspect-NetworkTools $root }
        'extract' { Prepare-Package $data $request.parameters }
        'enable-dns' { Enable-CloudflareDns $root $request.parameters }
        'restore-dns' { Restore-CloudflareDns $root }
        'start' { Start-GoodbyeDpi $root $data $request.parameters }
        'stop' { Stop-GoodbyeDpi $root }
        default { throw 'Ağ aracı işlemi geçersiz.' }
    }
    if ($mutation) { Write-ProtectedJson $response ([pscustomobject]@{ ok = $true; value = $value }) }
    else { Write-Json $response ([pscustomobject]@{ ok = $true; value = $value }) }
} catch {
    if ($response) {
        if ($mutation) { Write-ProtectedJson $response ([pscustomobject]@{ ok = $false; error = $_.Exception.Message }) }
        else { Write-Json $response ([pscustomobject]@{ ok = $false; error = $_.Exception.Message }) }
    }
    else { [Console]::Error.WriteLine($_.Exception.Message) }
    exit 1
}
