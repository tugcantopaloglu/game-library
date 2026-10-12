# Yerel kütüphane, Vencord ve güncellemeler

12 Ekim 2026 geliştirme kapsamı: elle oyun ekleme, koleksiyonlar, oyun ayrıntısı, kullanıcı başlık/not/kapak düzeltmeleri, oturum kaydı, kapak sağlayıcısı, yerel Discord/Vencord köprüsü ve güncelleme yönetimi.

## Kütüphane

Elle oyun ekle, seçilen `.exe` dosyasını ayrı bir kaynak olarak kaydeder. Başlatma shell komutu içermez; executable ve argümanlar ayrı tutulur. Oyun adına tıklamak ayrıntı ekranını açar. Başlık, not, kapak ve koleksiyon üyeliği sonraki launcher taramalarında korunur. Koleksiyonlar adları üzerinden oluşturulur, listelenir, filtrelenir ve silinir.

SQLite şema 3 kullanıcı düzeltmeleri, koleksiyonlar ve oturumları ekler. Şema 2 yükseltilmeden önce `library.schema-2-backup-<zaman>.db` oluşturulur; şema 1 yedekleme yolu korunur. Elle oyun ekleme son kaynak taramasının uyarılarını değiştirmez.

Oturumlar başlatma isteği, doğrulanmış çalışan süreç, sona eren süreç, başarısızlık, doğrulanamayan ve kesilmiş oturum olarak ayrılır. Manual/Epic/Riot için kayıtlı executable; Steam için ayrıntı ekranından seçilen, kurulum içindeki executable izlenir. Yeni süreç en fazla 60 saniye aranır; görüldükten sonra Windows süreç handle'ı üzerinden kapanış beklenir. Aynı anda sekiz oturum sınırı vardır. Uygulama kapanırsa sonraki açılışta açık oturumlar kesilmiş sayılır; oyunun kapandığı varsayılmaz.

## Kapaklar

Yerel JPEG/PNG seçimi veya izinli Steam/SteamGridDB CDN adresi kullanılabilir. İndirmeler HTTPS, izinli host, yönlendirme, 8 MiB dosya ve 4096×4096 piksel sınırlarından geçer. Görsel decode tahsisi 64 MiB ile sınırlandırılır. Kullanıcı kapakları `artwork/user`, çevrimiçi önbellek `artwork/cache` altında tutulur; önbellek 512 MiB ile sınırlanır. Kullanıcı kapakları otomatik tahliye edilmez.

SteamGridDB araması için API anahtarı Güncellemeler ekranında kaydedilir. Anahtar Windows Credential Manager'da saklanır; SQLite'a veya loglara yazılmaz. Steam kimliği varsa doğrudan o kimlikle, diğer oyunlarda başlıkla arama yapılır. Kullanıcı aday kapağı seçer; ad benzerliğiyle kapak otomatik uygulanmaz. Ağ kuyruğu tek işçi, tek aktif iş ve sınırlı kuyruk kullanır. API işleri başlangıçta en fazla iki istek/saniye ile seri çalışır; hata halinde sınırsız tekrar yapılmaz. [SteamGridDB API](https://www.steamgriddb.com/api/v2).

## Vencord

Kaynaklar [vencord.dev](https://vencord.dev/), [resmi installer](https://github.com/Vencord/Installer) ve [resmi Vencord deposudur](https://github.com/Vendicated/Vencord). Uygulama installer'ın son sürüm bilgisini GitHub API'den alır; Windows CLI asset'inin yayımlanan SHA-256 özetini ve PE başlığını doğrulamadan çalıştırmaz. Stable/PTB/Canary kullanıcı tarafından seçilir. Kurulum, onarım ve kaldırma ayrı işlemlerdir. Installer sonucunun başarılı olması canlı köprü bağlantısının doğrulandığı anlamına gelmez.

Standart Vencord installer özel `GameLibraryBridge` eklentisini içermez. Önce Vencord köprüsünü hazırla/güncelle kullanılır: resmi depo alınır, bu projenin eklentisi `src/userplugins` altına kopyalanır, bağımlılıklar lockfile ile kurulup tip kontrolü ve derleme yapılır. Bunun için Node.js 22+, pnpm, Git ve PowerShell 7 gerekir. Ardından Vencord kur/onar resmi installer'ı hazırlanan geliştirme derlemesine yönlendirir. Başarılı kurulumdan sonra mevcut Vencord ayarları yedeklenir; diğer eklentiler ve seçili kişiler korunarak GameLibraryBridge etkinleştirilir ve gerçek veri klasörü kaydedilir. Discord yeniden açılır, uygulamada yerel bağlantı açılır.

Geliştirici için bağımsız hazırlama komutu:

```powershell
pwsh -File scripts/prepare-vencord-bridge.ps1
```

Hazırlama Discord kurulumunu değiştirmez. İşlem günlükleri `%LOCALAPPDATA%\GameLibrary\integrations\vencord\prepare.log` ve `installer.log` altındadır. Özel Vencord kaynak derlemesi aynı dizinde korunur. Vencord'un kendi Git güncelleyicisi bu derlemeyi ve kullanıcı eklentisini birlikte yeniden derler; kaynak güncellemesi Node/pnpm kurulumu gerektirmeye devam eder.

Windows canonical yolunun `\\?\` öneki installer ve Electron'a verilmeden kaldırılır; UNC yolu normal UNC biçimine dönüştürülür. MSIX altında yönlendirilen kullanıcı veri dizini de eklentiye gerçek yoluyla yazılır. İlk canlı denemede bu önek Discord main-process `EISDIR/lstat C:` hatasına yol açtı. Orijinal Discord arşivi geri yüklenerek açılış doğrulandı, yol düzeltildikten sonra özel Vencord yeniden uygulandı. DOS/UNC dönüşümü regresyon testiyle korunur.

Köprü yalnızca loopback TCP üzerinden, oturum başına rastgele eşleştirme anahtarı ve protokol sürümüyle çalışır. Discord tokenı alınmaz. Bağlantı kapalıyken ana uygulamada bağlantı timer'ı açılmaz. Bağlantı koptuğunda kişi/oda verisi silinir; eski veri canlı gibi gösterilmez. Gösterilen arkadaş sayısı 50, oda sayısı 50, mesaj 256 KiB, komut kuyruğu 16 ile sınırlıdır. Kullanıcı eklentisindeki selectedFriends ayarı görüntülenecek kişileri sınırlar.

Köprü arkadaş durumu/aktivitesi, ses odaları, Discord'da görüşme açma, ses kanalına geçme, mute/deafen, destekleniyorsa yayın durdurma ve Vencord güncelleme komutlarını içerir. Normal kullanıcı hesabından otomatik DM göndermez. Yayın başlatma, kaynak, kalite/çözünürlük, FPS ve yayın sesi kontrolleri henüz gerçek istemcide doğrulanmadı; bu yetenekler kapalıdır. Teknik destek varmış gibi gösterilmez.

Canlı kabul: Discord Stable 1.0.9261 ve Vencord `16ec4ea838899706019e7f472e45f055070abeb1` derlemesinde eşleşmiş bağlantı kuruldu; 32 arkadaş, 1 ses odası ve etkin otomatik güncelleme tercihi okundu. Mute/deafen, ses kanalına geçiş, DM görüşmesi açma ve Vencord güncelleme fonksiyonları bulundu; bunların kullanıcı hesabında durum değiştiren işlemleri bu kabulde çalıştırılmadı. Yayın durdurma dahil bütün yayın yetenekleri `false` döndü. Kişi/oda içerikleri kabul raporunda saklanmaz.

## Uygulama güncellemesi

Güncelleme bildirimi sabit proje GitHub release kaynağından alınır. Yeni kararlı SemVer, izinli proje yolu, boyut, SHA-256, köprü protokolü ve Ed25519 imzası doğrulanır. İmzası olmayan veya eski sürüm reddedilir. Otomatik tercih etkinse en fazla günde bir kontrol edilir ve paket hazırlanır. Uygulama yeniden başlatılmadan executable değiştirilmez.

Yeniden başlat ve uygula, ayrı güncelleme yardımcısını başlatır. Yardımcı ana sürecin çıkmasını bekler, paketi yeniden doğrular, eski executable'ı yedekler ve yeni dosyayı atomik olarak yerleştirir. Yeni sürümün SQLite verisini/kapaklarını yükleyip software görünümü çizebilmesi gerekir. Yardımcı 30 saniye içinde doğru sürüm/PID ve rastgele açılış anahtarını içeren hazır raporunu bekler. Süreç erken kapanır veya hazır olmazsa yalnızca başlattığı yeni süreç sonlandırılır, önceki executable geri yüklenir ve yeniden açılır. Yeni sürümün veri geçişi başarısız olursa SQLite migration yedeği ayrıca korunur; executable geri dönüşü veritabanını otomatik geri almaz. Tam veri ve paket geri dönüşü ilk yayımlanmış sürüm zincirinde ayrıca doğrulanmalıdır.

Geliştirme derlemesinde yayımlanmış imzalı release ve güven anahtarı yoktur; canlı otomatik güncelleme bu yüzden açık sayılmaz. Yayın hazırlığı:

1. Ed25519 özel anahtarını depo dışında güvenli tut. Açık anahtarın ham 32 baytının base64 değerini `GAME_LIBRARY_UPDATE_PUBLIC_KEY` derleme değişkeniyle tanımla.
2. Release executable'ını derle.
3. `node scripts/sign-update.mjs <exe> <sürüm> <özel-anahtar.pem> <update.json>` ile bildirimi imzala.
4. İlk sürüm ve sonraki güncellemeyi gerçek paketle doğruladıktan sonra `v<sürüm>` release'ine executable ve `update.json` ekle. Bu geliştirme çalışması release yayımlamaz.

## Doğrulama araçları

`--scan-json --fixture-only` sadece verilen Steam fixture köklerini tarar; bilgisayardaki gerçek kurulumlar örnek veri setine karışmaz. `--search-benchmark <rapor>` 100 Rust sorgusu ölçer; UI gecikmesi içermez. `--startup-report <rapor>` yerel verinin/thumbnail'ların hazırlanması ve software snapshot süresini ölçer; işletim sistemi paint gecikmesi ayrı kalır. `--screenshot <png> --section <numara>` native görünümü kaydeder. `--stability-report <json>` varsayılan sekiz saat boyunca dakikalık checkpoint yazar; bilgisayar uyursa kesinti işaretlenir.

Gerçek oyun başlatma, Discord oturumu/yayın ve sekiz saat stabilite sonuçları ancak ilgili testin tamamlanmasıyla kabul edilir. Fixture veya tip denetimi gerçek bağlantı testi değildir.
