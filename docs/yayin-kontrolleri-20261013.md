# Yayın kontrolleri — 13 Ekim 2026

## Açık yayının kaynağını değiştirme

Aynı native seçici açık yayında Kaynak / Değiştir işlemini sunar. Mevcut çözünürlük ve FPS korunur. Kaynak yeniden okunurken hesap, oda, yayın anahtarı veya mevcut medya ayarları değişmişse istek uygulanmaz. Yeni kaynak kimliği geri okunmadan başarı gösterilmez. Kaynak doğrulama sürerken ikinci yayın işlemi kabul edilmez. İki kısa köprü testi kaliteyi koruma ve doğrulama sırasında başka yayına geçişi kontrol eder.

## Başlatma ve kaynak seçici

Native Yayın panelindeki kaynak seçici mevcut ses odasında paylaşılabilecek ekran ve pencereleri listeler. Discord'un kendi desktop capture köprüsünden yalnızca ad/kimlik alınır; thumbnail boyutu sıfırdır, ikon ve video önizlemesi istenmez. Liste kullanıcı açtığında veya Yenile seçtiğinde okunur. 100 kayıt ve 30 saniyelik seçim geçerliliği sınırı vardır. Panel kapandığında, sayfa değiştiğinde, tepsiye geçildiğinde ve hesap/oda/bağlantı değiştiğinde katalog bırakılır.

Başlatma isteği seçilmiş kaynak listesinin sürümü ve mevcut ses odasıyla eşleşmelidir. Kaynak uygulanmadan önce yeniden doğrulanır; kaybolmuş veya başlığı değişmiş pencere paylaşılmaz. Uygulama başka odaya otomatik katılmaz. Discord'un `STREAM_START` action sözleşmesi tanınırsa başlatma yeteneği açılır. Seçilen kaynakla gerçek aktif yayın geri okunmadan başarı gösterilmez.

Medya action'ı `desktopSettings`, `qualityOptions` ve `context` alanlarını kullanır; normalize edilmiş MediaEngineStore okuma nesnesi doğrudan action'a verilmez. Custom kalite preset'i istemciden bulunur. İstemci sözleşmesi tanınmazsa ilgili kontrol kapanır. Başlatma sırasında desteklenen kaynak için Sesi paylaş seçilebilir; başlangıç tercihi kapalıdır.

## Yayın sesi

Native panel yayın sesini açıp kapatır; mikrofon ve çağrı sesi kontrollerinden ayrıdır. Pencere sesi için SOUNDSHARE, ekran sesi için supportsScreenSoundshare desteği aranır. Soundshare süreç kimliği veya session değeri üretilmez; Discord'un medya action'ı bunları oluşturur. Sıfır değerli ses kimliği de etkin ses paylaşımı sayılır. Kaynak/kalite/FPS korunur ve gerçek ses durumu geri okunmadan başarı gösterilmez. Açık sesle desteklenmeyen kaynağa geçiş reddedilir; önce yayın sesi kapatılabilir. Video yakalama, ses yakalama ve kodlama Game Library sürecine taşınmaz.

Kaynaklar: [Discord istemci kodu kopyası](https://github.com/Discord-Datamining/Discord-Datamining/blob/master/current.js), [Vencord MediaEngineStore tipleri](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/MediaEngineStore.d.ts), [Electron desktopCapturer](https://www.electronjs.org/docs/latest/api/desktop-capturer). İstemci kodu kopyası canlı kurulumun sürüm uyumunun yerine geçmez.

Kısa kontrol: 12 köprü yayın testi, bir katalog/IPC bütçesi testi, üç native istek testi ve Vencord kaynağına karşı köprü TypeScript kontrolü geçti. Sınırlandırılmış kaynak metadata'sı, kapatılan seçicinin geciken cevabı, başlatmanın gerçek durumla doğrulanması, kaybolan pencere/hesap değişimi, kaynak/kaliteyi koruma, ses açma/kapatma ve action biçimi kontrol edildi. Kaynak katalogları 256 KiB IPC bütçesine sığdırılırken favoriler korunur; seçilen kaynak ID ve katalog sürümüyle doğrulandığından aktarımda listenin kısalması farklı pencereyi seçmez. Canlı Discord oturumu bulunmadığından gerçek yayın başlatılmadı. Yeni RAM/CPU veya uzun kararlılık ölçümü yapılmadı.

Commitler: başlatma `92c2620`, kaynak değiştirme `1b01334`, yayın sesi `ee7b5b9`. Node kontrolleri `node --import ./scripts/bridge-test-loader.mjs --test scripts/bridge-stream.test.mjs`, native istek kontrolleri `cargo test -p game-library-native stream::tests --locked` ile çalışır. Tam test paketi veya uzun performans testi çalıştırılmadı.

Son release derlemesi `cargo build -p game-library-native --release --locked` ile tamamlandı. `target/release/game-library-native.exe` ve `target/release/game-library-native-next.exe` aynı güncel binary'yi içerir; SHA-256: `54EF034A1CEAFB020BFBA72122C85A1116230F568E7F085DBB9BFDF3454668F8`. Native uygulama ve köprü birlikte güncellenmelidir. Gerçek Discord hesabında yayın/ses işlemi veya kurulum değişikliği yapılmadı.
