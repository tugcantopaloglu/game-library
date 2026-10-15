# Uygulamadan Discord köprüsü kurulumu

Windows x64 uygulaması derlenmiş GameLibraryBridge/Vencord paketini sıkıştırılmış olarak kendi içinde taşır. Kullanıcı bilgisayarında Node.js, Git, pnpm, PowerShell 7 veya Rust gerekmez. ZIP'ten uygulamayı çıkarmak ve **Ayarlar → Discord bağlantısı → Köprüyü kur / güncelle** düğmesine basmak yeterlidir. Stable, PTB ve Canary ayrı seçilir.

İlk kurulumda veya Discord yükleyicisi onarım gerektiriyorsa Discord kapalı olmalıdır. Uygulama açık Discord'u kapatmaz. Resmi Vencord kurucusu GitHub'dan indirilir; resmi release'in SHA-256 değeri ve indirme kaynağı doğrulanır. Kurucu hazır gömülü paketi kullanır; standart Vencord paketini indirip köprünün üzerine yazmaz. Normal köprü güncellemesinde kurucu veya internet gerekmez. Yeni kod sonraki Discord açılışında yüklenir.

Uygulama paketi boyut ve SHA-256 ile açar, eski köprü dosyalarını ve Vencord ayarlarını yedekler, dosyaları geçici dosya üzerinden değiştirir. İlk yamada Discord'un `app.asar` ve `_app.asar` dosyaları da yedeklenir. Kurucu başarı kodu döndürse bile yükleyicinin doğru `patcher.js` yoluna bağlandığı ve dört köprü dosyasının hash'leri ayrıca doğrulanır. Hata durumunda dosyalar, ayarlar ve kurulum kaydı geri alınır. Favoriler, diğer eklentiler ve temalar korunur. Yedekler `%LOCALAPPDATA%\GameLibrary\integrations\vencord\backups` altında kalır.

Başarılı ilk kurulumdan sonra **Uygulama güncellendiğinde köprüyü güncelle** açıksa uygulama her açılışta kurulu paketi bir kez kontrol eder. Daha yeni uygulamayla gelen paket veya bozulmuş dosyalar otomatik uygulanır; sürekli arka plan taraması yapılmaz. İlk kez kullanılan bilgisayara düğmeye basılmadan kurulum yapılmaz. Otomatik güncelleme tercihi kapatılabilir; Vencord kaldırıldığında da kapanır. Henüz yayımlanmamış uygulama sürümlerini indiren ayrı bir servis değildir: köprü, kullanıcının çalıştırdığı uygulamayla aynı sürümdedir.

## Geliştirme ve paketleme

Köprü kaynakları değişirse önce resmi Vencord checkout'undaki eklentiyi güncelle, `pnpm testTsc` ve `pnpm buildStandalone` çalıştır. Ardından:

```powershell
node scripts/package-vencord-bridge.mjs <resmi-vencord-checkout>
cargo build -p game-library-native --release --locked
```

`integrations/vencord/prebuilt/bundle.json` Vencord revision'ını, kaynak hash'lerini ve dört dosyanın hash/boyutunu içerir. Build script eski gömülü kaynakla uygulama derlenmesine izin vermez. Eski `--prepare-vencord` geliştirici yolu korunur; son kullanıcı düğmesi bunu çağırmaz.

`scripts/test-vencord-install.ps1` release executable'ı, yalnız alt sürecin APPDATA/LOCALAPPDATA ortamını ayrı fixture klasörlerine yönlendirerek resmi kurucuyla sınar. Gerçek Discord oturumu veya kullanıcı ayarlarına dokunmaz. Birim testleri bozuk paket, eksik kanal, geçersiz ayarlar, ilk kurulum, tekrar kurulum, açık Discord, dosya kilidi ve geri almayı kapsar. Canlı mesaj/yayın/hesap uyumluluğu diğer bilgisayardaki Discord oturumunda ayrıca test edilir.

Resmi kurucu sözleşmesi: [Vencord Installer](https://github.com/Vencord/Installer), [Windows kurulum davranışı](https://github.com/Vencord/Installer/blob/main/find_discord_windows.go), [paket yükleyicisi](https://github.com/Vencord/Installer/blob/main/app_asar.go).

## 15 Ekim 2026 doğrulaması

Son native kaynakla 75 birim testi ve 7 entegrasyon testi geçti; ağdan paket indiren isteğe bağlı probe atlandı. Sekiz yeni kurulum testi içinde gerçek Windows yol ayracı biçimi, yanlış yükleyici hedefi, bozuk paket, tekrar kurulum, otomatik güncelleme tercihi, ayarların korunması ve kilitli dosyadan sonra geri alma bulunur. All-targets Clippy (`-D warnings`) temizdir.

Resmi Vencord Installer 1.4.2, yalnız alt sürecin kullanıcı klasörleri yalıtılmış fixture'a yönlendirilerek çalıştırıldı. İlk kurulum, orijinal Discord arşivinin korunması, CommonJS paket sınırı, tekrar kurulumda değişiklik yapılmaması, kurucuyu yeniden çalıştırmadan köprü onarımı ve geçersiz ayarlarda kurulu dosyaların korunması doğrulandı. Gerçek Discord oturumu kullanılmadı. Kabul raporu `artifacts/bridge-auto-install-20261015/official-installer-acceptance.json` içindedir.

Hazır, hata, işlem sürüyor ve kurulu durumları %100/%125/%150 ölçeklerde görsel olarak kontrol edildi; dar pencerede işlem düğmeleri ve açıklamalar görünür, sağ arkadaş paneli yerinde kalır. Kurulum sürerken ilgili kontroller kilitlenir. Kabul görselleri aynı klasörde `ui-*-accepted.png` dosyalarındadır. `scripts/package-windows-test.ps1` gömülü paketi executable'dan yeniden doğrular ve uygulama/köprü kaynakları, lisanslar, yönerge ve SHA-256 listesiyle taşınabilir test ZIP'i üretir.
