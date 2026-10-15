# Native arayüz

13 Ekim 2026 sadeleştirmesi Rust/Slint ile derlenir; ana uygulamada WebView, React veya ağdan indirilen font bulunmaz. Önceki yenilemede [21st.dev Sidebar](https://21st.dev/@shadcn/components/sidebar), [Dashboard Sidebar](https://21st.dev/@arunjdass/components/dashboard-sidebar) ve [kart koleksiyonu](https://21st.dev/community/components/s/card) gezinme, yüzey ve bilgi hiyerarşisi için incelendi. Üçüncü taraf bileşen kodu kopyalanmadı; bileşenler Slint için yazıldı.

## Görsel sistem

Renkler ve ortak bileşenler `apps/native/ui/design.slint` içindedir. Yerel Segoe UI, grafit yüzeyler, ince ayrımlar ve açık mavi vurgu kullanılır. İkincil eylemler düz görünür; yalnızca odakta kenarlık gösterilir. Oyun adı düğme olarak gösterilmez.

| Token | Değer |
| --- | --- |
| Ana yüzey | `#101214` |
| Sol menü | `#14171A` |
| Panel | `#1B1F23` |
| Ayırıcı | `#2B3036` |
| Metin | `#ECF0F3` |
| İkincil metin | `#ADB5BE` |
| Vurgu | `#C6DAE8` |
| Vurgu yüzeyi | `#27333D` |
| Bağlantı durumu | `#8EC6A8` |
| Boşluklar | 4, 8, 12, 16, 20, 24, 32 px |
| Köşeler | Düğme 6, panel 8, kapak yaklaşık 7 px |
| Yazı | Gövde 13, yardımcı 11, sayfa 22, oyun başlığı 24 px |

SVG ikonları aynı çizgi dilini kullanır ve binary'ye gömülür. Software renderer'ın dikdörtgen klip sınırı nedeniyle kapak köşeleri küçük yerel SVG maskesiyle tamamlanır; kaynak görsel dosyası değiştirilmez. Sürekli animasyon veya ek zamanlayıcı kullanılmaz.

## Kütüphane

184 px sol menü kütüphane, favoriler ve koleksiyonları içerir; Discord ve Ayarlar alt alandadır. Platformlar tek açılır filtrede seçilir. Kaynaklar ve performans Ayarlar sekmelerinden açılır. Üst arama alanı Discord sayfasında arkadaşları, diğer sayfalarda oyunları arar. Oyun ekleme, tarama, kaynak yönetimi, tepsi ve çıkış üstteki ⋯ menüsünde bulunur; ayrı üst menü çubuğu kaldırıldı.

Kapaklar 2:3 oranında gösterilir. Pencere genişliğine göre 4, 5 veya 6 sütun seçilir. Başlık ve kaynak/boyut kapağın altındadır; uzun başlık kart genişliğini değiştirmez ve kesilerek gösterilir. Kurulum kullanılamıyorsa kapakta durum etiketi görünür ve başlatma kapalıdır. Kartın kapağı veya adı ayrıntıyı açar. Oynatma ve favori kontrolleri hover/odakta görünür; sağ tık menüsü ve klavye kısayollarıyla da erişilir. Seçili favorinin yıldızı doludur.

100 kayıtlık sayfa ve sanallaştırılmış satırlar korunur. Sütun sayısı değişince görünüm modeli yeni sütun sayısına göre gruplanır; filtre ve sayfa konumu korunur. Sayfalama yalnızca birden fazla sayfa varsa görünür. Boş favoriler ve boş aramalar farklı açıklama ve eylem içerir.

## Oyun ayrıntısı

144×216 px kapak, ad, platform, kurulum ve Oyna ilk alandadır. Tek Düzenle/Genel bakış eylemi iki görünüm arasında geçer. Genel bakışta üç ayrı HowLongToBeat kategorisi, gerçek eşleşme adı/kaynak, notlar ve oturumlar gösterilir. Süre bulunmuyorsa Veri yok; henüz alınmadıysa çizgi kullanılır. Oturumlar göreli zaman ve süreyle sunulur; Unix timestamp ve PID kullanıcı akışından çıkarılmıştır.

Düzenle bölümünde başlık/not kaydı, dosya/URL/SteamGridDB kapağı, koleksiyon üyeliği, HowLongToBeat eşleşmesi ve izlenecek executable vardır. Ayrıntıdan dönüş önceki kütüphane/favoriler görünümünü korur. Favori değişimi başlık/not düzenlemelerini yeniden yüklemez; ağdan gelen süreler yalnızca süre alanlarını günceller.

## Discord ve ayarlar

Discord sayfası bağlantı durumu, kullanılabilen ses kontrolleri, arkadaş satırları ve ses odalarını gösterir. Arkadaşlar en son konuşma üstte olacak şekilde sıralanır; bilinmeyen tarihler en sona gider. Görüşme zamanı yerel Discord kanal/okuma durumundaki son mesaj kimliğinden hesaplanır; mesaj içeriği alınmaz. Yıldız, sağ tık veya F ile arkadaş favoriye eklenir. Favoriler hesap bazında yerel olarak saklanır ve ayrı sekmede süzülür. Eski köprü sıralama verisi sağlayamıyorsa güncelleme hatırlatması görünür.

Bir arkadaş görüşmeyi Discord'da açar; ses odası ilgili katılma komutunu kullanır. İstemcinin desteklemediği yayın işlemleri etkin gösterilmez. Vencord kurulum/onarım/kaldırma ve elle ID alanı Ayarlar → Discord bağlantısı altına taşındı. Örnek IPC verisiyle alınan geliştirme görüntüleri “Örnek veri” olarak işaretlenir.

Ayarlar Genel, Kaynaklar, Discord bağlantısı ve Performans sekmelerine ayrılır. Kaynaklar içinde Steam/Epic/Riot sekmeleri bulunur. Kullanıcı hesap bağlantısını erteledi; arayüz bu tercihten bağımsız kullanılabilir. Kaynak tarama bildirimleri Discord'a taşınmaz.

## Klavye ve doğrulama

Kontroller native odak, erişilebilir etiket ve Enter/Space eylemi içerir. Ctrl+F aramaya odaklanır; kartta Enter ayrıntıyı, P başlatmayı ve F favoriyi çalıştırır. Escape ayrıntıdan geri döner. Canlı klavye/ekran okuyucu ve Windows DPI geçişi kabulü ayrıca bekliyor; görsel ölçek denemesi bu kabulün yerine geçmez.

Görsel kontroller gerçek yerel oyunların ayrı doğrulama profilinde yapılır; hesap anahtarı veya Discord kişi bilgisi örnek görsellerde yayımlanmaz. `--screenshot`, `--window-width`, `--window-height`, `--section`, `--detail-tab` ve `--query` ve `--skip-initial-scan` geliştirme kontrolleridir. Görüntüleme, kütüphane yüklemesinin bitmesini bekler ve 30 saniyede zaman aşımına uğrar. Ekran görüntüsü yanındaki `.layout.json` gerçek pencere boyutu, ölçek, sütun ve kayıt sayısını yazar. 10.000 kayıtlık fixture gerçek kullanıcı kütüphanesine karıştırılmaz.

Önceki RAM/CPU ölçümleri eski derlemeye aittir. Yeni tasarımın performansı ayrıca ölçülmeden aynı sonuçları sağladığı iddia edilmez. Önceki sekiz saat testinin raporu tamamlanmadı; sonraki kontrolde süreci çalışmıyordu.

## 12 Ekim yenilemesinin doğrulaması

Yeni release gerçek kullanıcı profiliyle açıldı; Discord bağlantısı yeniden kuruldu ve 32 arkadaş satırı okundu. Görüntüleme anında ses odası yoktu; uygulama oda uydurmaz. Mikrofon/ses kontrollerinin yetenekleri okundu; ses/DM/kanal eylemi bu görsel kontrolde tetiklenmedi.

Gerçek profilde 12 yerel kaydın 12 kapağı dosya olarak doğrulandı (Steam 8, Epic 1, Riot 3); 10 kayıt kurulu. Kullanıcı hesabı eşitlenmedi. Baldur's Gate 3 ve VALORANT süreleri gerçek profile de kaydedildi. 10.000 kayıtlık ayrı fixture, 100 kaydı 20 satırda yükledi. Normal/dar pencere, oyun ayrıntısı, boş arama, ayarlar, Discord ve Slint 1,5 ölçeği release binary ile görüntülendi. Bunlar yeni RAM/CPU veya uzun kararlılık ölçümü sayılmaz.

Kaynak commit, binary SHA-256, gerçek ekran boyutları ve kabul sınırları [doğrulama kaydında](../benchmarks/windows-native-metadata-and-ui-20261012.json).

13 Ekim sadeleştirmesinde release derlendi; kütüphane ve örnek Discord verisiyle 1280×720 mantıksal boyutta iki görüntü incelendi. Yeni tasarım için tam test paketi ve kaynak ölçümü tekrarlanmadı. Canlı Discord kabulü önceki doğrulamadan ayrıdır. [Çalışma kaydı](kontrol-20261013.md).

## 15 Ekim — denetimlerin yerleşimi ve klavye

Sohbet açılışında yayın paneli kapanır; Yayın düğmesiyle gerektiğinde yeniden açılabilir. Önceki mesajlar ve görünür kayıt sınırı mesaj listesinin üstündedir. Gelen ekler mesajın içinde, görsel önizleme ve 40 px açma düğmesiyle gösterilir. Kişi satırındaki okunmamış mesaj sayısı, presence noktası ve ses kanalı metninden ayrıdır; sabit sağ panelde de gösterilir.

Kütüphane, Discord ve Ayarlar için Ctrl+1/2/3; arama için Ctrl+F kullanılabilir. Tab/Shift+Tab gezinir, Enter/Boşluk denetimleri çalıştırır; Esc açık görüşmeyi veya oyun ayrıntısını kapatır. Oyun kartında F favoriyi, P kurulu oyunu başlatır. Favori yıldızı ayrı, adlandırılmış ve 40 px bir denetimdir. Arkadaş satırında odak çerçevesi ve okunmamış sayıyı içeren erişilebilir etiket vardır.

Eklenti kartlarında günlük işlemler ile sürüm yönetimi ayrı satırlardadır. Güncelle yerel paket klasörünü seçer; Geri dön düğmesinin erişilebilir adı hedef sürümü belirtir. Önceki sürüm bilgisi ve tercihlerin korunacağı metin olarak gösterilir. Var olan Theme, Action ve Panel bileşenleri kullanılır; ikinci bir stil sistemi eklenmez.

Dar pencerede ayar sekmeleri klavyeyle de kullanılabilen bölüm seçicisine dönüşür; sağdaki takip paneli ekranın dışına itilmez. Kısa pencerede sohbet aralıkları ve taslak yüksekliği azalır; Gönder satırı durum bildirimi açıkken de görünür kalır.
