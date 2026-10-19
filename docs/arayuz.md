# Native arayüz

19 Ekim 2026 görsel yenilemesi Rust/Slint ile derlenir; ana uygulamada WebView, React veya ağdan indirilen font bulunmaz. Google Stitch ile hazırlanan **Midnight Arcade Shelf** yönü gece mavisi yüzeyleri, lime eylemleri, lavanta yardımcı vurguları ve daha belirgin oyun rafını tanımlar. [Stitch projesi](https://stitch.withgoogle.com/projects/15089961104543268171), `9dda607a15734f3d81a880f50bc2793f` ekranı görsel referanstır; konseptteki örnek oyunlar, kişiler, sıralama seçenekleri ve diğer desteklenmeyen eylemler uygulamaya ürün verisi veya yeni özellik olarak taşınmaz.

Güncel tokenlar ve bileşen kuralları [native tasarım sisteminde](../apps/native/DESIGN.md) yer alır. Yanındaki [tasarım sidecar'ı](../apps/native/.impeccable/design.json) dokümantasyon önizlemelerini taşır; içindeki HTML/CSS ana uygulamanın çalışma zamanı değildir.

## Görsel sistem

Renkler ve ortak bileşenler `apps/native/ui/design.slint` içindedir. Yerel Segoe UI, gece mavisi yüzeyler, lime ana eylem ve lavanta yardımcı vurgu kullanılır. İkincil eylemler sakin yüzeylerdir; odak kenarlığı klavye konumunu gösterir. Oyun adı kapak altında metin olarak kalır.

| Token | Değer |
| --- | --- |
| Ana yüzey | `#101321` |
| Yan menüler | `#141829` |
| Panel | `#1B2137` |
| Ayırıcı | `#303953` |
| Metin | `#F2F4FA` |
| İkincil / yardımcı metin | `#B6BFD4` / `#929EB9` |
| Lime vurgu | `#D4F879` |
| Vurgu yüzeyi | `#293523` |
| Lavanta yardımcı vurgu | `#BEB2F4` |
| Çevrimiçi / kurulu | `#97DDB3` |
| Boşta / rahatsız etmeyin | `#F1C781` / `#F2A1AD` |
| Temel boşluklar | 4, 8, 12, 14, 16, 20, 28 px |
| Köşeler | Düğme 8, gezinme 10, panel ve kapak 12 px |
| Yazı | Gövde 13, yardımcı 11, sayfa 28 (kısa pencerede 24), oyun ayrıntısı 24, kart başlığı 14 px |

SVG ikonları aynı çizgi dilini kullanır ve binary'ye gömülür. Software renderer'ın dikdörtgen klip sınırı nedeniyle kapak köşeleri küçük yerel SVG maskesiyle tamamlanır; kaynak görsel dosyası değiştirilmez. Yüzeyler gölgesiz ton farkıyla ayrılır. Yenileme yeni font, runtime, sürekli animasyon veya ek zamanlayıcı getirmez.

## Kütüphane

Sol menü kütüphane, favoriler, koleksiyonlar ve platform filtrelerini içerir; Discord ve Ayarlar alt alandadır. Platformlar Tüm platformlar, Steam, Epic Games, Riot Games ve Elle eklenen olarak doğrudan seçilir. Tümü/Kurulu sekmeleri kurulum filtresidir. Kaynaklar ve performans Ayarlar bölümlerinden açılır. Üst arama alanı Discord sayfasında arkadaşları, diğer sayfalarda oyunları arar. Oyun ekleme, tarama, kaynak yönetimi, arkadaş paneli, tepsi ve çıkış üstteki ⋯ menüsünde bulunur.

Pencere 1100 mantıksal px altında sol menü 192 px, sağ arkadaş paneli 256 px ve içerik yan boşluğu 20 px'dir. Daha geniş pencerede bunlar 208, 280 ve 28 px olur. Arkadaş paneli kapatılınca kullanılabilir genişlik oyun rafına katılır. Minimum pencere 960×640, başlangıç tercihi 1280×720 mantıksal px'dir.

Pencere yüksekliği 720 mantıksal px altına düştüğünde üst arama bölümü 70 yerine 62 px, sayfa başlığı 28 yerine 24 px, içerik üst boşluğu 20 yerine 14 px ve ana içerik aralığı 16 yerine 12 px olur. Kütüphane başlığı altındaki yardımcı cümle bu kısa düzende gösterilmez; oyun rafına daha fazla alan kalır.

Kapaklar 2:3 oranında gösterilir. Kullanılabilir raf genişliği 16 px aralıkla 2–12 sütuna bölünür; işçi tarafındaki gruplama sınırı aynı aralıktadır. Başlık ve kaynak/boyut kapağın altındadır; uzun başlık kart genişliğini değiştirmez ve kesilerek gösterilir. Kurulum kullanılamıyorsa kapakta durum etiketi görünür ve başlatma kapalıdır. Kartın kapağı veya adı ayrıntıyı açar. Fare kapak üzerindeyken veya kart/eylemler odaktayken lime çerçeve ve kurulu oyunda **Oyna** alanı görünür. Favori denetimi de hover/odakta görünür; seçili yıldız sürekli görünür. Sağ tık menüsü ve klavye kısayolları aynı eylemlere erişir.

100 kayıtlık sayfa ve sanallaştırılmış satırlar korunur. Sütun sayısı değişince görünüm modeli yeni sütun sayısına göre gruplanır; filtre ve sayfa konumu korunur. Alt kayıt aralığı gerçek eşleşmeleri gösterir; Önceki/Sonraki yalnızca birden fazla sayfa varsa görünür. Boş favoriler ve boş aramalar farklı açıklama ve eylem içerir. Eksik kapakta gerçek oyun adı ve kaynakla yerel yer tutucu kullanılır.

## Oyun ayrıntısı

144×216 px kapak, ad, platform, kurulum ve Oyna ilk alandadır. Tek Düzenle/Genel bakış eylemi iki görünüm arasında geçer. Genel bakışta üç ayrı HowLongToBeat kategorisi, gerçek eşleşme adı/kaynak, notlar ve oturumlar gösterilir. Süre bulunmuyorsa Veri yok; henüz alınmadıysa çizgi kullanılır. Oturumlar göreli zaman ve süreyle sunulur; Unix timestamp ve PID kullanıcı akışından çıkarılmıştır.

Düzenle bölümünde başlık/not kaydı, dosya/URL/SteamGridDB kapağı, koleksiyon üyeliği, HowLongToBeat eşleşmesi ve izlenecek executable vardır. Ayrıntıdan dönüş önceki kütüphane/favoriler görünümünü korur. Favori değişimi başlık/not düzenlemelerini yeniden yüklemez; ağdan gelen süreler yalnızca süre alanlarını günceller.

## Discord ve ayarlar

Discord sayfası bağlantı durumu, kullanılabilen ses kontrolleri, arkadaş satırları ve ses odalarını gösterir. Arkadaşlar en son konuşma üstte olacak şekilde sıralanır; bilinmeyen tarihler en sona gider. Bu sıralama zamanı yerel Discord kanal/okuma durumundaki son mesaj kimliğinden hesaplanır; sıralama için mesaj içeriği gerekmez. Yıldız, sağ tık veya F ile arkadaş favoriye eklenir. Favoriler hesap bazında yerel olarak saklanır ve ayrı sekmede süzülür. Eski köprü sıralama verisi sağlayamıyorsa güncelleme hatırlatması görünür.

Arkadaş satırı native görüşmeyi açar; görüşmedeki ayrı eylem destekleniyorsa Discord'a geçer. Mesaj okuma/gönderme kapsamı [sohbet sözleşmesinde](chat.md) tanımlıdır. Ses odası ilgili katılma komutunu kullanır. İstemcinin desteklemediği yayın işlemleri etkin gösterilmez. Vencord kurulum/onarım/kaldırma ve elle ID alanı Ayarlar → Discord bağlantısındadır.

Sağ arkadaş paneli takip edilen kişilerin gerçek presence, oyun ve ses bilgisini gösterir. Çevrimiçi yeşil, boşta amber, rahatsız etmeyin pembe ve çevrimdışı/bilinmeyen nötr renktir; metin etiketi durumu açıklar. Okunmamış sayı ve favori ayrı kalır. Bağlantı kesilince son bilinen durum olduğu belirtilir, oyun/ses bilgisi güncelmiş gibi sunulmaz. Boş panel açıklaması içeriği kadar yükseklik kaplar. Geliştirme fixture'ları “Örnek veri” ve sağ panelde “Örnek kişiler · görsel doğrulama” olarak etiketlenir; gerçek kullanıcı profiline karıştırılmaz.

1280 px altındaki genişlikte açık görüşme varsa genel Discord sayfa başlığı gizlenir; görüşmenin kişi adı görünür kalır. Böylece dar düzende mesaj alanına yer ayrılır. Sohbetin 900 px altındaki yükseklik düzeni ve 850 px altında yayın panelini kapatma davranışı korunur.

Ayarlar Genel, Kaynaklar, Discord bağlantısı, Entegrasyonlar, Ağ erişimi, Performans ve Eklentiler bölümlerine ayrılır. Kaynaklar içinde Steam/Epic/Riot sekmeleri bulunur. 1280 px altındaki pencere genişliğinde ayarlar sekmeleri bölüm seçicisine dönüşür. Kullanıcı hesap bağlantısını erteledi; arayüz bu tercihten bağımsız kullanılabilir. Kaynak tarama bildirimleri Discord'a taşınmaz.

## Klavye ve doğrulama

Kontroller native odak, erişilebilir etiket ve Enter/Space eylemi içerir. Ctrl+1 kütüphane, Ctrl+2 Discord ve Ctrl+3 ayarları açar. Ctrl+F kütüphane aramasına odaklanır; kartta Enter/Boşluk ayrıntıyı, P kurulu oyunu başlatmayı ve F favoriyi çalıştırır. Escape ayrıntıdan geri döner veya açık görüşmeyi kapatır. Canlı klavye/ekran okuyucu ve Windows DPI geçişi kabulü ayrıca bekliyor; görsel ölçek denemesi bu kabulün yerine geçmez.

Görsel kontroller gerçek yerel oyunların ayrı doğrulama profilinde yapılır; hesap anahtarı veya Discord kişi bilgisi örnek görsellerde yayımlanmaz. `--screenshot`, `--window-width`, `--window-height`, `--section`, `--detail-tab` ve `--query` ve `--skip-initial-scan` geliştirme kontrolleridir. Görüntüleme, kütüphane yüklemesinin bitmesini bekler ve 30 saniyede zaman aşımına uğrar. Ekran görüntüsü yanındaki `.layout.json` gerçek pencere boyutu, ölçek, sütun ve kayıt sayısını yazar. 10.000 kayıtlık fixture gerçek kullanıcı kütüphanesine karıştırılmaz.

Önceki RAM/CPU ölçümleri eski derlemeye aittir. Yeni tasarımın performansı ayrıca ölçülmeden aynı sonuçları sağladığı iddia edilmez. Önceki sekiz saat testinin raporu tamamlanmadı; sonraki kontrolde süreci çalışmıyordu.

## 12 Ekim yenilemesinin doğrulaması

Yeni release gerçek kullanıcı profiliyle açıldı; Discord bağlantısı yeniden kuruldu ve 32 arkadaş satırı okundu. Görüntüleme anında ses odası yoktu; uygulama oda uydurmaz. Mikrofon/ses kontrollerinin yetenekleri okundu; ses/DM/kanal eylemi bu görsel kontrolde tetiklenmedi.

Gerçek profilde 12 yerel kaydın 12 kapağı dosya olarak doğrulandı (Steam 8, Epic 1, Riot 3); 10 kayıt kurulu. Kullanıcı hesabı eşitlenmedi. Baldur's Gate 3 ve VALORANT süreleri gerçek profile de kaydedildi. 10.000 kayıtlık ayrı fixture, 100 kaydı 20 satırda yükledi. Normal/dar pencere, oyun ayrıntısı, boş arama, ayarlar, Discord ve Slint 1,5 ölçeği release binary ile görüntülendi. Bunlar yeni RAM/CPU veya uzun kararlılık ölçümü sayılmaz.

Kaynak commit, binary SHA-256, gerçek ekran boyutları ve kabul sınırları [doğrulama kaydında](../benchmarks/windows-native-metadata-and-ui-20261012.json).

13 Ekim sadeleştirmesinde release derlendi; kütüphane ve örnek Discord verisiyle 1280×720 mantıksal boyutta iki görüntü incelendi. Yeni tasarım için tam test paketi ve kaynak ölçümü tekrarlanmadı. Canlı Discord kabulü önceki doğrulamadan ayrıdır. [Çalışma kaydı](kontrol-20261013.md).

13 Ekim tasarım referansı: [21st.dev Sidebar](https://21st.dev/@shadcn/components/sidebar), [Dashboard Sidebar](https://21st.dev/@arunjdass/components/dashboard-sidebar) ve [kart koleksiyonu](https://21st.dev/community/components/s/card) gezinme, yüzey ve bilgi hiyerarşisi için incelendi. Üçüncü taraf bileşen kodu kopyalanmadı; bileşenler Slint için yazıldı. O tarihteki grafit/açık mavi renkler, 184 px menü ve 4–6 sütun düzeni 19 Ekim görsel sistemiyle değişti.

## 15 Ekim — denetimlerin yerleşimi ve klavye

Sohbet açılışında yayın paneli kapanır; Yayın düğmesiyle gerektiğinde yeniden açılabilir. Önceki mesajlar ve görünür kayıt sınırı mesaj listesinin üstündedir. Gelen ekler mesajın içinde, görsel önizleme ve 40 px açma düğmesiyle gösterilir. Kişi satırındaki okunmamış mesaj sayısı, presence noktası ve ses kanalı metninden ayrıdır; sabit sağ panelde de gösterilir.

Kütüphane, Discord ve Ayarlar için Ctrl+1/2/3; arama için Ctrl+F kullanılabilir. Tab/Shift+Tab gezinir, Enter/Boşluk denetimleri çalıştırır; Esc açık görüşmeyi veya oyun ayrıntısını kapatır. Oyun kartında F favoriyi, P kurulu oyunu başlatır. Favori yıldızı ayrı, adlandırılmış ve 40 px bir denetimdir. Arkadaş satırında odak çerçevesi ve okunmamış sayıyı içeren erişilebilir etiket vardır.

Eklenti kartlarında günlük işlemler ile sürüm yönetimi ayrı satırlardadır. Güncelle yerel paket klasörünü seçer; Geri dön düğmesinin erişilebilir adı hedef sürümü belirtir. Önceki sürüm bilgisi ve tercihlerin korunacağı metin olarak gösterilir. Var olan Theme, Action ve Panel bileşenleri kullanılır; ikinci bir stil sistemi eklenmez.

Dar pencerede ayar sekmeleri klavyeyle de kullanılabilen bölüm seçicisine dönüşür; sağdaki takip paneli ekranın dışına itilmez. Kısa pencerede sohbet aralıkları ve taslak yüksekliği azalır; Gönder satırı durum bildirimi açıkken de görünür kalır.

## 19 Ekim — Stitch görsel yenilemesi

Görsel yön Google Stitch'te üretildi, çalışan bileşenler mevcut Rust/Slint sistemine uygulandı. Platform filtreleri sol menüye taşındı; kütüphane rafı arkadaş panelinin açık/kapalı durumuyla kullanılabilir alana göre 2–12 sütun seçer. Hover ve odakta Oyna alanı, kalıcı seçili favori, anlamlı arkadaş durum renkleri ve ayrı örnek veri etiketi güncel tasarım sözleşmesidir.

Doğrulama tamamlandı:

- `cargo build -p game-library-native --release --offline --locked`: uyarısız release derlemesi.
- `cargo clippy -p game-library-native --all-targets --offline --locked -- -D warnings`: geçti.
- `cargo test -p game-library-native --offline --locked`: 81 birim ve 7 entegrasyon testi geçti; resmi paket indiren 1 isteğe bağlı test atlandı.
- Ayrı test profillerinde yedi ekran incelendi: kütüphane 1440×960, 1280×800 ve 960×640; boş arama 1280×800; ayarlar 960×640; örnek sohbet 960×640 ve 1440×960. Görünür kütüphanede 24 kayıt 4/3/2 sütuna ve 6/8/12 satıra doğru gruplandı.
- Sekiz klavye kontrolü ve altı aşamalı kapak yaşam döngüsü geçti. Görünür alan dışında kapak tutulmadı; gizleme ve yeniden açma doğrulandı. Arayüz hata günlükleri boştu.
- Görsel incelemedeki dört düzeltme doğrulandı: panel ayırıcısı, simge/sayaç hizaları, boş arkadaş alanının aralığı ve 960×640 ilk satırının başlık/kaynak/boyut görünürlüğü. Dar sohbet alanı tam bir mesajı yazarı ve saatiyle gösterebiliyor.

Yerel kanıtlar `artifacts/stitch-review-20261019/final/` altındaki `report.json`, `interface.json`, `cover-lifecycle.json` ve PNG dosyalarıdır. Doğrulanan release SHA-256: `FB66249419C28B0EC79A95F21DE7D5FF7D2D10F042DB56C5ABA36386BC981A42`.

Görsel fixture, açıkça örnek adlar ve Stitch konseptinden alınan test görsellerini kullanır; bunlar uygulamayla dağıtılmaz. Kapak yaşam döngüsü ayrı 480×720 çıktılı sentetik kapaklarla kontrol edildi. Gerçek kullanıcı profili veya canlı Discord oturumu kullanılmadı. Tek ekran ve 1,0 ölçek denendi; farklı DPI/çoklu ekran, ekran okuyucu, yeni RAM/CPU ve uzun süreli kararlılık sonucu bu çalışmanın kapsamına dahil değildir. Yukarıdaki 12–15 Ekim kayıtları kendi derlemelerine aittir.
