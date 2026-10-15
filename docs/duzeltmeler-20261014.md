# 14 Ekim 2026 düzeltmeleri

Kullanıcı yayın panelinin, Discord sohbetinin ve tam Steam/Epic hesap listesinin çalışmadığını bildirdi. Önceki tamamlanma özeti bu özelliklerin canlı kabulünü olduğundan ileri göstermişti. Bu rapor kaynak kodu, üretilen dosya ve gerçek oturum sonucunu ayrı tutar.

Kontrol edilen eski `game-library-native.exe` ve `game-library-native-next.exe` aynı SHA-256 önekine sahipti (`54ef034a1ceafb02`). İkisinde de `chat_open` yoktu. Son sohbet/entegrasyon commitinin kaynak kodu bu executable'larda bulunmuyordu.

Discord adaptörlerinin biçime bağlı kontrolleri gevşetildi; bilinmeyen birebir açma sözleşmesine string kullanıcı kimliği gönderilmez. Önceden doğrulanan birebir kanal, yeni kanal açma adaptörü bulunmasa da iç sohbet için kullanılabilir. Yayın paneli artık destek algısı başarısız olduğunda gizlenmez; başlat, durdur, çözünürlük ve FPS kontrolleri görünür kalır ve kullanılabilirliklerine göre etkinleşir.

Komut iletimi Rust'ın snapshot okuma turundan ve Vencord'un saniyelik poll'undan ayrıldı. TCP komutu renderer'daki bekleyen IPC isteğini uyandırır. Store olayları anlık snapshot gönderimini tetikler; native durum değişikliği GUI işçisine doğrudan bildirilir. Heartbeat ve eskimiş veri kontrolü korunur. Bu, zamanlayıcı beklemesini kaldırır; Discord istemcisinin gerçek işlem süresi ayrıca ölçülmelidir.

Favori arkadaşlar tüm ekranlara eklenen sağ panelde gösterilir. Göster/gizle tercihi saklanır. Profil görseli, çevrimiçi/boşta/rahatsız etmeyin durumu, paylaşılan oyun ve izinle görülebilen ses odası birlikte gösterilir. Bağlantı kesildiğinde son bilinen durum güncel diye gösterilmez. Avatarlar yalnız Discord CDN'den ayrı işçide alınır; 64×64 decode, 256 KiB dosya, 256 görsel bellek ve 256 dosya disk sınırı vardır.

Steam giriş yönlendirmesi sırasında yok olan tarayıcı execution context'i için yeniden bağlanma eklendi. Store tokenı/yanıtı kullanılamıyorsa aynı özel tarayıcı oturumundaki kendi Steam Community oyun listesine geçilir; başka hesabın listesi reddedilir. Eksik/geçersiz listeler kaydedilmez. Epic listesi `--force-refresh` ile alınır. Epic komut satırı girişi artık listeyi SQLite'a kaydeder. Başarılı elle eşitleme sonrasında arama, kurulu/favori/koleksiyon filtreleri temizlenip ilgili kaynağın oyunları gösterilir.

Genel yerel eklenti yöneticisi ve executable/stdio JSON-RPC host'u eklendi. Sürüm, kimlik, yetenek, paket sınırları, etkinleştirme, klasör seçimi, süreç grubu, zaman aşımı ve kütüphaneye uygulama vardır. [Protokol ve örnek paket](eklenti-sistemi.md).

Bu çalışma ortamında Discord/Vencord kurulumu ve Steam/Epic hesap oturumu bulunmadığından gerçek sohbet, yayın, hesap sayıları, profil fotoğrafı ve mikrofon gecikmesi canlı doğrulanamaz. Fixture ekranları gerçek oturum kanıtı değildir. Yeni sürümün RAM/CPU veya sekiz saat kararlılık sonucu bu kayıtla kabul edilmez.

Gerçek CDN denemesinde `cdn.discordapp.com` TLS bağlantısı hem native istemcide hem PowerShell/Python'da başarısız oldu. Güncel köprüde avatarlar Discord'un Electron ağ yolundan istenir; yalnız PNG/JPEG görsel verisi sınırlı paketle native decode işçisine aktarılır. Mevcut oturum/yol sorunu uygulamanın ayrı HTTP istemcisini etkilediğinde bu yol Discord'un kendi erişimini kullanır. Bütün yollar başarısızsa sağ panel açık bir fotoğraf yükleme uyarısı gösterir; ağ ayarları otomatik değiştirilmez. Bu ortamda gerçek Discord hesabı olmadığı için Electron yolu canlı doğrulanamadı.

Legendary `--force-refresh` ve liste seçenekleri [upstream CLI](https://github.com/legendary-gl/legendary/blob/master/legendary/cli.py) üzerinden kontrol edildi; resmi Windows yardımcı dosyası yayımlanan SHA-256 ile indirilip seçenekleri gerçek executable'dan okundu.

## Yerel Windows oturumunda devam doğrulaması

Güncel uygulama ve Vencord köprüsü aynı kaynakla derlenip eşlendi; kurulu köprü dosyaları yedeklendi ve hash'leri karşılaştırıldı. Discord yeniden açıldıktan sonra 32 arkadaş ve 2 ses odası, native arayüzde bağlantı ve ses kontrolü kullanılabilirliği doğrulandı. Önceki ortamda canlı oturum bulunmamasıyla ilgili sınır bu yerel Windows kontrolü için geçerli değildir.

Arkadaş paneli açıkken kart genişliği hesabı panelin 280 pikselini çıkarmıyordu. Kartlar artık kullanılabilir alana göre hesaplanır. Kapak decode sınırı 180×270 yerine 480×720 oldu; Lanczos3 küçültmesi kullanılır, küçük kaynak görseller büyütülmez. Dosya ve decode bellek sınırları korunur. Arkadaş ve oda durum noktaları büyütüldü, avatarın sağ alt köşesine bağlandı; bağlantı noktaları 10 piksel ve esnemeyen boyutlara sahip.

Sohbet adaptörü Vencord'un `proxyLazy` fonksiyon biçimindeki modül nesnelerini reddediyordu. Yalnız istemci action adaptörleri bu nesneleri kabul eder; komut ve mesaj verilerinin nesne doğrulaması korunur. Regresyon testi gerçek callable proxy biçimini kapsar. Canlı kontrolde Discord'da arkadaşa tıklamadan bir birebir görüşmenin son 30 mesajı yüklendi ve `canSend: true` gözlendi. Kontrol sırasında mesaj gönderilmedi; gerçek gönderim/nonce geri okuma kabulü ayrıca bekliyor.

18 sohbet testi, kapak boyut/dosya sınırı testi ve Vencord tip kontrolü geçti. Bu oturumda `snyk_code_scan` aracı sunulmadığından istenen Snyk taraması yapılamadı. Önceki RAM/CPU ölçümleri daha yüksek çözünürlüklü kapaklar için geçerli kabul edilmez.

Release derlemesi tamamlandı. 540 kayıtlı kopya profille 960/1280 piksel ve %150 ölçekli 1920×1080 görüntüler incelendi; arkadaş paneli açıkken üç/dört sütun taşmıyor. Son bağlantı noktası metinle aynı satır merkezinde. Görseller örnek arkadaş içerir; gerçek sohbet kabulü ayrı `artifacts/ui-chat-fix-20261014/chat-live.json` kaydındadır.

## Sohbet kaydırması, seçim ve geniş pencere düzeni

Kullanıcı gerçek mesaj gönderiminin çalıştığını doğruladı; son mesaja kayma, arkadaşın birkaç tıklamada açılması ve büyük penceredeki oranları bildirdi. Sohbetin en fazla 30 mesajı artık tahmini satır boyu kullanan sanal ListView yerine gerçek satır yüksekliğiyle ScrollView içinde yerleşir. Görüşme değişiminde, gönderme isteğinde ve doğrulanan nonce'ta son mesaj takip edilir. Görünür yükseklik değiştiğinde takip korunur; eski mesajlara elle kaydırıldığında Son mesaja git seçeneği bulunur. Mesaj balonları 800 piksel ile sınırlandı, saat sağa hizalandı.

Native köprü güncellemeleri arkadaş, takip edilen arkadaş, oda ve mesaj modelini her snapshot'ta değiştirmiyor. Aynı model üzerinde yalnız değişen satırlar güncellenir; etkileşim bileşenleri korunur. Aynı kişiye tekrar tıklamak devam eden/yüklenmiş görüşmeyi yeniden açmaz. Yeni görüşme isteği önceki görüşmenin revizyonunu taşımaz; hesap ve arkadaş doğrulaması korunur. Gönderme/yenileme/kapatma işlemleri kendi kanal ve revizyonuyla sınırlandırılmaya devam eder. Eski seçime ait UI geri dönüşü yeni seçimi değiştirmez.

Kütüphane 3–12 sütunla kullanılabilir genişliğe göre yerleşir; 2560 piksel pencerede arkadaş paneli açıkken 10 sütun olur. Paneller içerik yüksekliğinde kalır. 850 pikselden kısa pencerede sohbet açılırken yayın paneli daraltılır; Yayın düğmesinden yeniden açılabilir.

9 native sohbet testi ve model kimliği/satır güncelleme regresyon testi geçti. `--chat-fixture --chat-scroll-fixture` 30 değişken yükseklikte örnek mesajla son mesaja kaymayı görsel olarak doğrular; gerçek Discord mesajı göndermez. Görseller ve ölçüleri `artifacts/chat-layout-20261014/` altında tutulur. Snyk aracı bu oturumda da sunulmadı.


## 15 Ekim 2026 — sohbet kurtarma, kapak belleği ve arayüz

Sohbet açma isteğinin hesap, kişi, başlangıç revizyonu ve son tarihi native tarafta izlenir. Yeni Discord revizyonundaki hata yüklemeyi bitirir; başarısız istek aynı kişide Tekrar dene sunar. Komut daha gönderilmeden reddedilirse hata korunur; önceki görüşmenin durum güncellemeleri bu hatayı örtemez. Hesap/kişi değişiminde eski istek bırakılır. Durum snapshot'ı değişmese de yerel son tarih kontrol edilir. Mesaj gönderme ve nonce onayı değişmez.

100 kapaklık sayfa metadata olarak kalır; kapak çözme işi ayrı işçide yalnız görünür satırlar ve sınırdaki bir satır için çalışır. 480×720 üst sınırı ile Lanczos3 çıktısı değişmedi; test piksel dizisini önceki yöntemle birebir karşılaştırır. Küçük görseller büyütülmez ve dosya/decode sınırları korunur. 32 MiB önbellek ve UI, SharedPixelBuffer üzerinden aynı RGBA tahsisini kullanır. Kaydırma, filtre, sayfa, sekme ve tepsi değişiminde ekran dışı görsel referansları bırakılır. Tek bekleyen iş değiştirilir; eski işin sonucu yeni sayfaya uygulanmaz. Kapak indirme tamamlanınca ekran dışındaki satır hemen çözülmez.

Mevcut Slint tasarımı `impeccable` polish/optimize ve `make-interfaces-feel-better` rehberleriyle korundu: sohbet başlığı ile eylemleri iki satıra ayrıldı, mesaj balonları geniş pencerede doğru tarafa hizalandı, hata ve tekrar deneme görünür oldu. Sohbet ve favori eylemlerinde 40 px hedefler, kapaklarda nötr ince sınır ve düğmelerde basılı durum geri bildirimi eklendi. Örnek veri gösteriminde gerçek bağlantı düğmesi gösterilmez.

Yeni kabul çıktıları `artifacts/fix-review-20261015/` altında tutulur. Canlı Discord hesabında mesaj gönderimi bu değişiklikte çalıştırılmadı; otomatik testler ve ayrı örnek profilde native UI doğrulaması kullanılır.

62 native Rust testi, 55 Node köprü testi ve `cargo clippy -p game-library-native --all-targets --offline --locked -- -D warnings` geçti. Önbellek testinde aynı dosyanın piksel tamponu adresi paylaşılır; resize çıktısı önceki algoritmayla byte düzeyinde eşittir. Mekanik Impeccable taraması bulgu döndürmedi; Slint için asıl görsel doğrulama native software renderer ekran görüntüleridir.

1280×860 örnek profilde 100 farklı 600×900 kaynak kapak kullanıldı. Görünür bantta 12 tam 480×720 kapak 15,82 MiB RGBA tuttu; önceki tüm sayfayı çözme politikasının 131,84 MiB hesabına göre %88 daha azdır. Bunlar kapak piksel belleği değerleridir, toplam süreç RAM'i değildir. Kaydırma sonrası ekran dışı görsel sayısı sıfır, tepside modeldeki görsel sayısı sıfır, yeniden açılışta son satırın dört kapağı aynı çözünürlükteydi. Önbellek üst sınırı 32 MiB; çok geniş pencerede görünür görsellerin toplamı bu hedefi aşabilir, kalite bu sınırı tutturmak için düşürülmez.

Son release dosyasıyla 960×720 sohbet, 1280×860 hata/kütüphane ve 2560×1080 geniş kütüphane görüntüleri doğrulandı; runtime hata/uyarı logları boştu. `cover-lifecycle.json` release akışında da `passed: true` döndürdü. Aynı 100 kapaklı örnek profilde 10,03 saniyelik kısa pencere ölçümü: ortalama özel çalışma kümesi 26,14 MiB, private bytes 27,37 MiB ve makine toplamına göre CPU %0,026; rapor kesintisiz ve tek süreçlidir. Bu kısa ölçüm, 60 saniye performans veya uzun kararlılık kabulü yerine geçmez. Ham rapor: `artifacts/fix-review-20261015/window-memory.json`.
