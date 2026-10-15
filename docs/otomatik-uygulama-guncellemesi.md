# Otomatik uygulama güncellemesi

Game Library, yeni kararlı Windows x64 sürümlerini bu projenin [GitHub Releases](https://github.com/tugcantopaloglu/game-library/releases) sayfasından alır. Yeni kurulumlarda otomatik güncelleme açıktır. Önceden kaydedilmiş kapalı tercihi korunur. **Ayarlar → Genel → Uygulama güncellemeleri** bölümünden tercih değiştirilebilir, elle kontrol/indirme yapılabilir ve hazır güncelleme hemen uygulanabilir.

Otomatik kontrol başarılıysa 24 saat, bağlantı veya indirme başarısızsa 15 dakika sonra yeniden denenir. Zaman bilgisi diskte tutulur; uygulamanın tekrar açılması bu süreyi sıfırlamaz. Başka bir entegrasyon çalışıyorsa kontrol sıranın boşalmasını bekler. İndirme ilerlemesi ayarlarda görünür. Güncelleme mevcut oturumu kesmez: sonraki normal açılışta kurulur. Otomatik tercih kapalıysa indirilen paket de yalnızca **Yeniden başlat ve güncelle** düğmesiyle uygulanır.

## Doğrulama ve geri dönüş

GitHub API'den son kararlı release ve asset bilgisi alınır. Sürüm etiketi, bildirimin sürümü ve executable'ın sürüme özel indirme adresi birebir eşleşmelidir. Prerelease, sürüm düşürme, farklı depo/yol, uyumsuz köprü protokolü, 80 MiB üzeri executable ve 16 KiB üzeri bildirim kabul edilmez. İndirme yönlendirmeleri yalnız GitHub'ın izinli HTTPS asset sunucularını kullanır.

`release/update-public-key.txt` uygulamaya derlenir. Bildirimdeki sürüm, adres, SHA-256, boyut ve protokol alanları Ed25519 ile doğrulanır. Çalıştırmadan önce dosyanın SHA-256, boyut ve Windows başlığı yeniden denetlenir. Yarım indirmeler hazır güncelleme olarak işaretlenmez. Hazır paket ve bildirim atomik yazılır; `updates/pending.json` en son kaydedilir.

Yardımcı süreç paketi denetleyip hazır olduğunu bildirmeden uygulama kapanmaz. Yardımcı ana sürecin çıkmasını bekler, executable'ı ve SQLite veritabanını yedekler. SQLite `VACUUM INTO` yedeği WAL içindeki kaydedilmiş verileri de içerir; yardımcı şema geçişi çalıştırmaz. Yeni executable aynı kurulum klasöründe atomik değiştirilir.

Yeni sürümün 30 saniye içinde kütüphaneyi yükleyip arayüzü çizebilmesi gerekir. Sağlık raporu doğru sürüm, başlatılan PID ve rastgele tek kullanımlık anahtarı taşımalıdır. Başarısızlıkta yalnız bu yeni süreç kapatılır; executable ve kütüphane veritabanı birlikte geri alınır, eski uygulama yeniden açılır. Başarısız sürüm/hash kaydı otomatik kurulum döngüsünü engeller. Kullanıcı kapakları, diğer uygulamalar ve Discord süreçleri bu işlemde değiştirilmez.

Veriler ve günlükler `%LOCALAPPDATA%\GameLibrary\updates` altındadır: `helper.log`, `result.json`, `pending.json`, `schedule.json`; sürüm klasöründeki `previous.exe` ve `library.previous.db` kurtarma yedekleridir. Uygulama dosyasının bulunduğu klasör kullanıcı tarafından yazılabilir olmalıdır.

## Release yayımlama

Bir defalık Ed25519 özel anahtarı depo dışında saklanır. Bu bilgisayardaki kopya `%LOCALAPPDATA%\GameLibrary\release-signing\update-private.pem` içindedir ve kullanıcıya özel Windows klasör izinleriyle korunur. GitHub Actions'ın şifreli `UPDATE_SIGNING_KEY` repository secret'ı aynı PEM anahtarını içerir. Özel anahtar arşivlenmez, loglanmaz ve Git'e eklenmez. Bu anahtarın kaybı mevcut istemcilerin yeni imzaları doğrulamasını engeller; güvenli bir yedeğini sakla. `scripts/create-update-key.mjs` mevcut anahtarı yeniden kullanır; açık anahtar farklıysa kendiliğinden döndürmez.

1. `apps/native/Cargo.toml` sürümünü ve `Cargo.lock` kaydını güncelle, `release/v<SÜRÜM>.md` sürüm notunu ekle.
2. Değişiklikleri test edip commit et. Aynı commit'e `v<SÜRÜM>` etiketi ekleyip GitHub'a gönder.
3. `.github/workflows/release-windows.yml` Windows üzerinde sabit Rust toolchain ile test, Clippy ve release derlemesini çalıştırır. Etiket/sürüm/commit eşleşmesini kontrol eder.
4. Eşleşen Vencord kaynaklarıyla Windows ZIP'i oluşturur, bildirimi imzalar. Derlenen uygulamanın kendi doğrulayıcısıyla paketi ve gerçek UI açılışındaki sürümü denetler.
5. EXE, `update.json`, Windows ZIP'i ve Vencord kaynak arşivi önce draft release'e yüklenir. Tüm yüklemeler bitince release kararlı/latest olarak yayımlanır. Yayımlanmış sürümler değiştirilmez; düzeltme yeni sürümle çıkar.

GitHub Actions ekranından **Windows release → Run workflow** ile mevcut sürüm etiketi için başarısız yayın yeniden denenebilir. Tamamlanmış release'in üzerine yazılmaz. Yerel eşdeğeri, temiz commit ve derlenmiş executable varken:

```powershell
pwsh -File scripts/build-release.ps1 -Tag v0.2.2 -PrivateKeyFile "$env:LOCALAPPDATA\GameLibrary\release-signing\update-private.pem"
```

Bu komut yalnızca imzalı/doğrulanmış paketi hazırlar. `-Publish` ayrıca mevcut Git etiketi ve GitHub yetkisiyle draft/yayın adımlarını çalıştırır. İmzalama betiği kullanılan özel anahtarın uygulamadaki açık anahtara uyduğunu denetler; yanlış anahtarla sessizce release üretmez.

## İlk geçiş ve kontroller

0.1.0 deneme paketlerinde güven anahtarı derlenmemişti. Bu paketler kendilerini yeni sürüme taşıyamaz; 0.2.2 veya üzerindeki ilk paketi bir kez açmak gerekir. Kütüphane verileri aynı kullanıcı veri klasöründen alınır. Bundan sonraki sürümler için ZIP taşıma gerekmez.

```powershell
cargo test -p game-library-native -p library-core -p platform-windows --locked
node --test scripts/sign-update.test.mjs
cargo clippy -p game-library-native -p library-core -p platform-windows --all-targets --locked -- -D warnings
```

Testler imza ve sürüm/yol eşleşmesini, bozuk/kalıcı hazır paketleri, kontrol takvimini ve WAL yedeğini kapsar. Windows test istemcisiyle gerçek executable değiştirme, sağlıklı açılış, yanlış sürüm, açılış çökmesi, bozuk veritabanından geri alma ve eski sürecin yeniden başlatılması ayrıca sınanır. `--update-command status|check|download|apply|enable|disable|verify`, `--folder`, `--output` ve `--data-dir` seçenekleri izole kabul profillerinde aynı üretim akışını kullanır.
