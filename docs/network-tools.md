# İsteğe bağlı ağ erişimi araçları

Cloudflare DNS ve GoodbyeDPI ayrı ayrı etkinleştirilir. Uygulamanın açılması, Discord bağlantısının başarısız olması veya bir paketin güncellenmesi ağ ayarlarını değiştirmez. Ağ değişikliği ve GoodbyeDPI başlatma/durdurma eylemleri kullanıcı düğmesine basınca normal Windows UAC penceresini açar. İzin reddedilirse işlem gerçekleşmez.

## Cloudflare DNS

Ayarlar → Ağ erişimi ekranında bağdaştırıcı seçimi kalıcı Windows interface GUID değerine dayanır. Bağdaştırıcı adı veya sonradan değişebilen interface index kimlik olarak kaydedilmez. Varsayılan rota bağdaştırıcısı önerilir; kullanıcı başka bir bağdaştırıcı seçebilir.

Standart Cloudflare resolver adresleri:

| Aile | Birincil | İkincil |
| --- | --- | --- |
| IPv4 | `1.1.1.1` | `1.0.0.1` |
| IPv6 | `2606:4700:4700::1111` | `2606:4700:4700::1001` |

DoH destekleyen Windows sürümlerinde şifreli seçeneği `https://cloudflare-dns.com/dns-query` tanımını, otomatik HTTPS yükseltmesini ve UDP'ye geri dönmeme tercihini uygular. DoH desteklenmezse açık hata döner; kullanıcı şifresiz DNS seçeneğini ayrıca seçebilir. Windows'un DoH sunucu tanımları sistem genelindedir: aynı Cloudflare sunucusunu kullanan diğer bağdaştırıcılara da HTTPS yükseltmesi uygulanabilir. Mevcut bağdaştırıcı DoH registry ayarları ve diğer DNS sağlayıcılarının tanımları değiştirilmez. Grup ilkeleri, tarayıcıların kendi DNS ayarları veya işletim sistemi politikaları gerçek DNS trafiğini etkileyebilir; yapılandırma sonucu trafiğin şifrelendiği iddiası değildir.

Değişiklikten önce her adres ailesinin otomatik/statik DNS kaynağı, özgün statik adres sırası, etkin DNS adresleri ve değiştirilecek dört Cloudflare DoH tanımı saklanır. Özgün IPv4 otomatik, IPv6 statik ise geri alma bunları ayrı ayrı eski durumuna döndürür; yalnız mevcut adresleri yazıp DHCP'yi statik hale getirmez. Kullanılabilir olmayan adres ailelerine dokunulmaz; iki aile de okunamıyorsa etkinleştirme reddedilir. IP adresi, DHCP IP kaynağı, VPN, proxy ve güvenlik duvarı ayarları değiştirilmez.

Başarısız etkinleştirme hemen eski ayarları geri almaya çalışır. Geri alma kısmen başarısız olursa korunan özgün kayıt saklanır ve yeniden deneme mümkündür. Uygulama yeniden açıldığında aynı kayıt okunur. Bağdaştırıcı kaldırılmışsa başka bağdaştırıcıya uygulama yapılmaz. Tam oturumda uygulanan değerlerin, yarım oturumda ise uygulanan veya önceden geri yüklenmiş özgün değerlerin hâlâ mevcut olduğu doğrulanır; başka bir programın sonradan yaptığı DNS/DoH değişikliği ezilmez. Arayüz geri alma düğmesini `rollbackAvailable` alanına göre etkinleştirir; bu alan yarım kalan kurtarma kaydını da kapsar. Bozulmuş kullanıcı preference/prepared.json dosyaları korunan runtime/recovery inspection'ını engellemez.

Cloudflare DNS bir indirilen GitHub aracı değildir; DNS adresleri resmi Cloudflare dokümantasyonundan sabit olarak tanımlanır. Cloudflare adına bir GitHub updater veya WARP kurulumu kullanılmaz.

## GoodbyeDPI ve güncelleme

Kaynak yalnız [ValdikSS/GoodbyeDPI](https://github.com/ValdikSS/GoodbyeDPI) deposunun kararlı GitHub release kanalıdır. `releases/latest` API'sinden etiket, dosya adresi, boyut ve varsa GitHub asset `sha256:` digest alanı alınır. Sürüm karşılaştırması SemVer ile yapılır. Taslak/prerelease, yanlış depo, farklı HTTPS host/port, beklenmeyen dosya/yol ve daha eski sürüm reddedilir. Release indirme yönlendirmeleri yalnız GitHub'ın bilinen release asset host ve path'lerine kabul edilir; API başka hosta yönlenemez.

13 Ekim 2026'daki doğrulamada kararlı sürüm `0.2.2`, paket `goodbyedpi-0.2.2.zip` ve yayımlanmış GitHub asset digest alanı boştu. Sürüm uygulamaya sabitlenmez; gelecekteki kararlı sürüm dinamik sorgulanır. Yayımlanmış digest varsa eşleşmeyen dosya kesinlikle reddedilir. Digest yoksa HTTPS kaynağı doğrulanır ve yerel SHA-256 kayıtları indirmeden sonra değiştirilmiş dosyaları algılar; bu kayıtlar bağımsız yayıncı imzası yerine geçmez. `publisher_digest_verified` bu ayrımı raporlar.

İndirme en fazla 32 MiB, açılmış toplam içerik en fazla 64 MiB ve 512 ZIP entry ile sınırlıdır. Yol geçişi, sürücü yolu, alternate data stream, sembolik bağlantı, Windows aygıt adı, büyük sıkıştırma oranı ve büyük/küçük harfe göre yinelenen dosyalar reddedilir. İndirilen `.cmd` dosyaları çalıştırılmaz. İşletim sistemi mimarisine uygun `goodbyedpi.exe`, `WinDivert.dll` ve WinDivert sürücüsü bulunmalıdır; ARM64 paketi bu entegrasyonda doğrulanmadığından reddedilir.

Paket hazırlama ve güncelleme kontrolü yönetici izni istemez, DNS değiştirmez ve hiçbir paket dosyasını çalıştırmaz. Otomatik güncelleme başlangıçta kapalıdır. Kullanıcı açarsa `auto_update_if_due` en fazla 24 saatte bir resmi sürümü kontrol eder, yeni sürümü benzersiz bir klasöre hazırlar ve mevcut çalışan süreçte değişiklik yapmaz. İndirme hatası ve son kontrol zamanı tercihlerde görünür. Yeni sürüm kullanıcı mevcut oturumu durdurup yeniden başlatınca kullanılır.

Kullanıcı GoodbyeDPI'ı başlatınca aynı sürümün resmi ZIP'i ilk kez yönetici helper'ında HTTPS üzerinden yeniden alınır; executable bileşenleri doğrudan bu byte dizisinden türetilir. Kullanıcı alanındaki manifest ile dosya hash'lerinin birlikte değiştirilmesi, bir saldırgan executable dosyasını yönetici olarak çalıştırmaya izin vermez. Bileşenler ve authoritative proof kaydı, nesneler oluşturulurken atomik Administrators sahipliği/ACL verilen korunan klasöre yazılır. Sonraki başlatmalar bu korunan proof'u internet gerektirmeden tekrar hash ile doğrulayarak kullanır. Yeni sürüm için ilk doğrulama yeniden resmi indirme gerektirir. WinDivert sürücüsünün Windows Authenticode imzası ayrıca denetlenir. Belgelenen `-6` profili kullanılır; shell argümanı veya dışarıdan verilen script çalıştırılmaz. GoodbyeDPI ağ paketlerini WinDivert kernel sürücüsüyle işler; yönetici izni bu nedenle gerekir. Harici bir GoodbyeDPI süreci veya başka uygulamaya ait WinDivert sürücüsü saptanırsa mevcut oturum değiştirilmeden başlatma reddedilir. Kendi önceki WinDivert sürücü kaydı yeni paketin sürücüsünden farklı bir hash'e bağlıysa eski sürücüyle yeni paket başlatılmaz; bu kernel sürücü geçişi ayrı kabul/kurulum doğrulaması gerektirir.

Durdurma yalnız uygulamanın korunan kaydındaki PID, executable yolu, başlangıç zamanı ve süreç sahibi birlikte eşleşirse yapılır. Kimlik denetiminden önce process handle sabitlenir; sonlandırma kontrol edilmiş Process nesnesinin `Kill()` metodu üzerinden yapılır. Yeniden kullanılan PID veya farklı sahibin süreci sonlandırılmaz. Uygulama çıkınca/çökünce bir oturum gözcüsü aynı çocuğu durdurur. Gözcü aynı handle'ları kullanır ve tamamlanınca kapatır. UAC launcher `Start-Process -Wait` ile çocuk ağacını beklemez; yalnız yükseltilmiş helper'ın kendi `WaitForExit()` sonucunu bekler. Uygulama dışı kalıcı GoodbyeDPI Windows servisi, başlangıç girdisi, antivirüs istisnası veya güvenlik duvarı değişikliği kurulmaz. WinDivert'in kendi talep üzerine açtığı sürücü kaydı/bileşeni Windows'ta görülebilir; dış WinDivert servisleri silinmez veya durdurulmaz. Çalışıyor durumunun ağdaki engelin gerçekten kaldırıldığını doğrulamadığı arayüz sonucunda belirtilir.

## Saklama ve elevation

- İndirilen paketler ve kullanıcı tercihi: `<data>/network-tools`.
- Yönetici korumalı DNS geri alma ve süreç kaydı: `%ProgramData%/GameLibraryNetwork/<kullanıcı SID>/<data yolu hash>`.
- Korunan klasörün sahibi Administrators, yazma hakkı yalnız Administrators/System, çağıran kullanıcı için okuma hakkıdır. Yerel, domain ve Entra kullanıcı SID'leri desteklenir.
- Yönetici eyleminin helper kodu native binary içine gömülür. UAC bootstrap kodu dosyayı yazma/silme paylaşımı kapalı read handle ile, gömülü byte uzunluğu sınırında bir kez okur; gömülü SHA-256 ile doğrular ve aynı byte dizisinden oluşturulan ScriptBlock'u çalıştırır. Kullanıcı tarafından değiştirilen helper dosyası çalıştırılmaz.
- Yükseltilmiş helper işlem yanıtını kullanıcı tarafından yazılabilen klasöre yazmaz; administrator-owned `responses` klasöründeki sınırlı önbelleğe yazar. İlk çağıran kullanıcı haklarında kalan dış launcher bu yanıtı kullanıcı alanına kopyalar; böylece kullanıcı alanındaki hardlink/junction yarışı yönetici dosya yazmasına dönüşmez.
- Shell executable kullanıcının `PATH`/ortam değişkeninden alınmaz; korunan HKLM Windows `SystemRoot` kaydı kullanılır. Parametreler JSON/Base64 verisidir; shell metnine ham yol, GUID veya paket verisi eklenmez. Dış unelevated launcher `-Command`, inner UAC bootstrap `-EncodedCommand` kullanır. Dış source'u da UTF-16/Base64 yapmak resmi `0.2.2` üç bileşenli paket ve normal LocalAppData yolunda 35.008 command-line UTF-16 birimi üretiyordu; düzeltilmiş çağrı 13.192 birimdir (NUL dahil, uzun PID sınırıyla). Quote/escape ve UTF-16 hesabı spawn öncesi yapılır; 32.767 sınırını aşan özel yol/payload çalıştırılmaz.
- Yol doğrulamaları reparse/junction hedeflerini reddeder. Korunan metadata dizininin yönetici dışı sahibine/yazma hakkına rastlanırsa işlem reddedilir.

## Native API

| Çağrı | Sonuç ve kullanım |
| --- | --- |
| `inspect(data)` | `Report`; `summary()` kısa UI metni, `runtime.interfaces` GUID/ad/durum/aile listesi, `preferredInterfaceGuid`, `dohSupported`, `dnsEnabled`, `rollbackAvailable`, `dnsError`, `running`, `runningVersion` |
| `check_update(data)` | `Result<Option<Release>, String>`; yeni kararlı sürüm veya yok |
| `prepare_latest(data)` | `Result<Package, String>`; son kararlı paketi hazırlar veya mevcut paketi döndürür |
| `enable_dns(data, guid, encrypted)` | UAC ile tek seçilmiş bağdaştırıcıda snapshot + Cloudflare DNS |
| `restore_dns(data)` | UAC ile korunan özgün snapshot'ı geri yükler |
| `start(data)` / `stop(data)` | UAC ile uygulamanın GoodbyeDPI oturumunu başlatır/durdurur |
| `set_auto_update(data, enabled)` | Kullanıcının güncelleme tercihini kaydeder |
| `auto_update_if_due(data)` | Opt-in ve 24 saat şartını kendi uygular; arka plan worker çağırabilir; `true` yeni paket hazırlandığını belirtir |

Mutasyon dönüşleri JSON `message` alanı içerir. Eylem tamamlandıktan veya UAC reddedildikten sonra UI `inspect` ile gerçek durumu yeniden okur. `Report.preferences` başlangıçta kapalı `auto_update`, `last_check` ve `last_update_error` alanlarını taşır. `runtime.stopAvailable` korunan süreç kaydının okunabildiğini belirtir; farklı admin credential kullanıldığında normal kullanıcı `running` durumunu doğrulayamazsa bile korunan kayıt üzerinden güvenli stop/cleanup istenebilir. Yeni paket hazırlanması çalışan sürümü otomatik yeniden başlatmaz.

## Doğrulama

```powershell
cargo test -p game-library-native --test network_tools
& "$env:SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe" -NoProfile -File scripts/native-network-tests.ps1
# İsteğe bağlı resmi paket indirme/staging probe; paket çalıştırılmaz.
cargo test -p game-library-native --test network_tools official_stage_probe -- --ignored --nocapture
```

Rust harness saf kaynak/sürüm/digest/path doğrulamaları, resmi paket metadata'sıyla 32k komut sınırı regresyon kontrolü, bozuk mutable metadata'dan bağımsız inspection ve değişiklik yapmayan gerçek Windows inspection içerir; 7 test geçti. PowerShell testleri 58 assertion ile ZIP traversal/duplicate, DHCP IPv4 + statik IPv6 rollback, yarım DNS/DoH geri alma sırasında drift, authoritative ZIP'ten bileşen türetme, korunan proof'un offline kullanımı, değiştirilmiş executable'ın reddi, in-memory ACL sahiplik/hak descriptor'ları, süreç sahibi/PID tekrar kullanımı ve guardian parent exit davranışını mock'larla doğrular. Atomic ACL için .NET Framework FileStream/DirectoryInfo security overload'larının Windows PowerShell 5.1 üzerinde mevcut olduğu ayrıca doğrulandı. Resmi kullanıcı alanı staging probe `0.2.2` paketiyle ayrıca geçti. Korunan authoritative proof'un gerçek UAC/start akışı, canlı DNS değişikliği, WinDivert yükleme, ağdaki Discord erişimi ve app-exit guardian kabul testleri bu geliştirme sırasında çalıştırılmadı; kullanıcının seçtiği cihazda ayrıca doğrulanmalıdır. Oturumda `snyk_code_scan` tool'u bulunmadığından zorunlu Snyk taraması gerçekleştirilemedi.

Birincil kaynaklar: [Cloudflare Windows kurulumu](https://developers.cloudflare.com/1.1.1.1/setup/windows/), [Cloudflare IP adresleri](https://developers.cloudflare.com/1.1.1.1/ip-addresses/), [Cloudflare DoH endpoint](https://developers.cloudflare.com/1.1.1.1/encryption/dns-over-https/make-api-requests/), [GoodbyeDPI release kanalı](https://github.com/ValdikSS/GoodbyeDPI/releases), [kararlı 0.2.2 README ve profiller](https://github.com/ValdikSS/GoodbyeDPI/blob/0.2.2/README.md), [Microsoft DNS adreslerini değiştirme/reset](https://learn.microsoft.com/en-us/powershell/module/dnsclient/set-dnsclientserveraddress?view=windowsserver2025-ps), [Microsoft DoH AutoUpgrade/UDP fallback](https://learn.microsoft.com/en-us/powershell/module/dnsclient/set-dnsclientdohserveraddress?view=windowsserver2025-ps).
