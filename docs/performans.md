# Performans ve kararlılık

## Başlangıç bütçesi

Bu rakamlar tasarım ve kabul hedefleridir; bütün bilgisayarlarda garanti değildir. İlk sürümün ölçümleri [ilk sürüm belgesinde](ilk-surum.md) kaydedilir. Windows 11 x64, SSD, release derlemesi, 1.000 oyun ve 1440p ekran başlangıç referansı; gerçek cihaz ve kullanılan veri seti rapora yazılacak.

| Senaryo / ölçüt | Hedef |
| --- | --- |
| Kütüphane açık, veri yüklenmiş, boşta | Tüm uygulama süreçlerinde toplam özel çalışma kümesi ≤ 180 MiB |
| Pencere kapalı, tepsi açık, sosyal bağlantı kapalı | ≤ 40 MiB |
| Tepsi açık, Discord yardımcısı bağlı | Toplam ≤ 70 MiB; SDK etkisi ayrıca raporlanır |
| Boşta CPU | 60 saniyede makine toplamına göre ortalama < %0,25 |
| Boşta disk/ağ | Uygulamadan periyodik tarama/metadata isteği yok; gerekli bağlantı heartbeat'i hariç |
| Yerel veriden ilk kullanılabilir ekran | p95 ≤ 1 saniye |
| Sıcak yeniden açılma | p95 ≤ 500 ms |
| 10.000 kayıtta arama yanıtı | Rust sorgusu + IPC p95 ≤ 50 ms |
| Kaydırma | 60 Hz ekranda p95 frame aralığı ≤ 16,7 ms; > 50 ms uzun görev yok |
| Görsel indirme | En fazla 2 eşzamanlı iş |
| Görsel disk önbelleği | Varsayılan 512 MiB |
| UI görsel çalışma kümesi | Native sayfa görselleri için 32 MiB hedef |

Ana native uygulama şu anda tek süreçtir. Discord veya eklenti yardımcısı eklendiğinde bunlar da toplam bütçeye alınacak ve süreç üyeliği sayacı genişletilecek. Önceki Tauri karşılaştırmasında uygulamaya ait WebView renderer/utility/GPU süreçleri de toplandı. Özel çalışma kümesi, private bytes/commit ve toplam çalışma kümesi birlikte kaydedilir.

GPU motor kullanımı ve dedicated/shared GPU belleği ayrıca izlenecek. Boşta sürekli render olmaması hedef; WebView/GPU sürücüsünün taban tüketimi deneyde ölçülecek. RAM bütçesi düşük tutulurken VRAM'e kontrolsüz yük aktarılmayacak.

## Çalışma politikası

- Dosya olayları yalnızca manifest köklerinde; olaylar birleştirilip kaynak bazında yeniden işlenir.
- Açılışta önce SQLite snapshot'ı gösterilir; keşif ve ağ işleri sonradan başlar.
- UI sadece görünür satırları render eder. İlk native sürüm 100 kayıtlık sayfanın JPEG/PNG kapaklarını en fazla 180×270 piksele indirir; tam sayfanın RGBA verisi en fazla 18,54 MiB'dir. Decode işçi thread'inde, tek görsel üzerinde yapılır. Daha büyük kütüphanelerde görünür kapaklara göre yükleme sonraki optimizasyondur.
- Tam boy artwork, blur ve arka plan video kullanılmaz. Hareketler kısa, etkileşime bağlı ve durabilir.
- Mevcut Vencord eklentisi tek timer ile saniyede bir, en fazla 50 kişi/50 oda snapshot'ı gönderir; native görünüm açıkken iki saniyede bir yenilenir. Kişi başına timer yoktur. Olay tabanlı snapshot optimizasyonu sonraki iştir; eklentinin Discord sürecine ek maliyeti henüz ayrı ölçülmedi.
- SQLite bağlantı/cache boyutları sınırlı; iş kuyrukları sınırlı ve iptal edilebilir.
- Oyun başladığında ağır keşif ve metadata kuyruğu duraklatılır. Kullanıcı açıkça devam ettirebilir.
- Başlatılan oyun süreçleri öncelikle OS olaylarıyla izlenir. Kaçırılan olayları düzeltmek için yalnızca ilgili süreçlerde seyrek ve sınırlı kontrol uygulanabilir.
- Native pencere kapatılıp tepsiye geçildiğinde çizim durur; görünüm modeli korunur. Hem doğrudan tepsi açılışı hem kullanılmış pencerenin kapanması ayrı ölçülür.

## Ölçüm protokolü

Rapor istenen süreyle birlikte gerçek ölçüm süresini ve en uzun örnek aralığını kaydeder. CPU ortalaması geçen süreye göre ağırlıklandırılır. 2,5 saniyeden uzun örnek aralığı veya hedef süreden belirgin sapma `interrupted` olarak işaretlenir; `scripts/measure.ps1` bu kaydı karşılaştırma için kabul etmez. Ham rapor korunur ve kesintisiz ölçüm tekrarlanır.

1. Debug araçları kapalı release derlemesini ölç; Node/Vite geliştirme sunucusunu sonuçlara katma.
2. Soğuk açılış, sıcak açılış, 60 saniye boşta, yoğun kaydırma, arama, tarama ve tepsi senaryolarını ayır.
3. 100, 1.000 ve 10.000 oyun; kapaklı/kapaksız; Discord kapalı/açık veri setlerini karşılaştır.
4. Açılış ve aramayı en az 20 kez ölç; p50/p95 ve en kötü sonucu kaydet.
5. Windows süreç ölçümleri ve gerektiğinde WPR/WPA ile CPU, disk ve GPU kaynaklarını doğrula.
6. 50 pencere aç/kapat döngüsü ve 8 saat boşta çalışma sonunda kalıcı büyüme veya yeni orphan süreç olmadığını kontrol et.
7. Oyun kapalı/açık halde kontrollü A/B ölçüm yap; arka plan işlerinin frame-time üzerindeki etkisini kaydet.

İlk benchmark arayüz + SQLite + bir Steam adaptörüyle yapılacak. Kaynak bütçesi geçilirse önce görünür kapaklar, WebView yaşam döngüsü, sorgular ve arka plan işleri incelenir. Buna rağmen geçilemiyorsa daha fazla özellik eklenmeden aynı iş yüküyle Rust/Slint karşılaştırılır. Hedef sessizce yükseltilmez.

Native tam sürümün önceki 1.000 kayıtlı kapaksız/kapaklı/tepsi ölçümleri ve 50 pencere döngüsü tamamlandı; RAM/CPU bütçeleri karşılandı. Ölçüm özeti ve doğrulanmayan alanlar [benchmark kaydında](../benchmarks/windows-native-20261012.json) yer alır. Yeni modüllerin etkisi aynı yöntemle tekrar ölçülür; geçmiş yaşam döngüsü testi yeni modüllerin kabulü yerine geçmez.

Epic/Riot sonrası 2.004 kayıtlı sürüm pencere açıkken 32,32 MiB / %0,199; doğrudan tepsi açılışında 24,06 MiB / %0,028 ölçüldü. Her iki 60 saniyelik kayıt tek süreçli ve kesintisizdi. [Yeni kaynak benchmark kaydı](../benchmarks/windows-native-sources-20261012.json).

Kullanıcı kütüphanesi/Vencord sonrası release, 10.000 kapaklı fixture kaydında pencere 29,84 MiB / %0,011; doğrudan tepsi 25,09 MiB / %0,004 ölçüldü. Bu iki kesintisiz 60 saniyelik ölçümde köprü kapalıydı; Discord'un ve eklentinin kaynakları dahil değil. 100 çekirdek aramasında p95 15,93 ms ölçüldü; UI/IPC gecikmesi henüz dahil edilmedi. 20 ayrı süreç açılışında thumbnail yükleme ve software snapshot dahil p95 661,95 ms; dosya sistemi cache'i ortaktı, OS paint gecikmesi ölçülmedi. Önceki ölçümden veri seti ve bilgisayar farklı olduğu için doğrudan hızlanma iddiası yapılmaz. Son cihaz Windows 11 Pro, Ryzen 5 7500F ve 31,73 GiB RAM. [Yeni benchmark ve kalan kabul işleri](../benchmarks/windows-native-user-library-20261012.json).

## Kararlılık senaryoları

| Durum | Beklenen davranış |
| --- | --- |
| Bozuk / yarım manifest | Kaynak hatası gösterilir; diğer oyunlar korunur |
| Tarama iptali | Yarım snapshot mevcut oyunları kaldırmaz |
| Disk çıkarılması | Kurulum erişilemiyor olur; kullanıcı verileri kalır |
| Launcher güncellemesi | Kaynak tekrar doğrulanır; kayıtlar isimden yeniden üretilmez |
| Oyun başlatma hatası | Başarısız oturum olarak kaydedilir; uygulama kapanmaz |
| İnternet kesintisi | Yerel kütüphane çalışır, ağ kuyruğu gecikmeli bekler |
| Discord kapanması | Sosyal durum bilinmiyor olur; bağlantı kontrollü tekrar denenir |
| Yetki reddi | İlgili yetenek kapanır; tekrar giriş seçeneği kalır |
| SDK/eklenti çökmesi | Yardımcı süreç sınırlı tekrar denenir; çekirdek çalışır |
| Veritabanı geçişi hatası | Yedek korunur; eski verinin üstüne yazılmaz |
| İkinci uygulama açılışı | Yeni süreç mevcut pencereyi öne getirip çıkar |

Testler parser, kaynak eşleştirme, silinme koşulları, migration, launcher komutları ve kritik bağlantı durumlarını kapsayacak. Görsel stil ayrıntılarını tekrar eden testler yazılmayacak. Windows yaşam döngüsü ve gerçek launcher davranışı yalnızca tarayıcı testleriyle doğrulanmış sayılmayacak.
