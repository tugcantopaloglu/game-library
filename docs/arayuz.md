# Native arayüz

12 Ekim 2026 yenilemesi Rust/Slint ile derlenir; ana uygulamada WebView, React veya ağdan indirilen font bulunmaz. [21st.dev Sidebar](https://21st.dev/@shadcn/components/sidebar), [Dashboard Sidebar](https://21st.dev/@arunjdass/components/dashboard-sidebar) ve [kart koleksiyonu](https://21st.dev/community/components/s/card) gezinme, yüzey ve bilgi hiyerarşisi için incelendi. Üçüncü taraf bileşen kodu kopyalanmadı; bileşenler Slint için yazıldı.

## Görsel sistem

Renkler ve ortak bileşenler `apps/native/ui/design.slint` içindedir. Yerel Segoe UI, grafit yüzeyler, ince ayrımlar ve soluk lavanta vurgu kullanılır. Düğmeler birincil, ikincil ve düz varyantlara ayrılır; genişlikleri içerik kadardır. Oyun adı düğme olarak gösterilmez.

| Token | Değer |
| --- | --- |
| Ana yüzey | `#111216` |
| Sol menü | `#16171C` |
| Panel | `#1B1D24` |
| Ayırıcı | `#2B2E38` |
| Metin | `#F2F2F6` |
| İkincil metin | `#A1A5B2` |
| Vurgu | `#B9ADFF` |
| Vurgu yüzeyi | `#2B263F` |
| Bağlantı durumu | `#8ECDB1` |
| Boşluklar | 4, 8, 12, 16, 20, 24, 32 px |
| Köşeler | Düğme 8, panel 12, kapak yaklaşık 7 px |
| Yazı | Gövde 13, yardımcı 11, bölüm 30, oyun başlığı 32 px |

SVG ikonları aynı çizgi dilini kullanır ve binary'ye gömülür. Software renderer'ın dikdörtgen klip sınırı nedeniyle kapak köşeleri küçük yerel SVG maskesiyle tamamlanır; kaynak görsel dosyası değiştirilmez. Sürekli animasyon veya ek zamanlayıcı kullanılmaz.

## Kütüphane

224 px sol menü kütüphane, favoriler, koleksiyonlar ve platformları içerir. Discord, Ayarlar ve Performans alt alandadır. Platforma tıklamak ilgili oyunları gösterir. Hesap/klasör ayarlarına kütüphanenin filtre satırındaki dişliden erişilir. Üst arama alanı bütün sayfalarda bulunur; düzenleme anında kütüphaneye döner. Oyun ekleme ve tarama üst çubukta tek yerde durur.

Kapaklar 2:3 oranında gösterilir. Pencere genişliğine göre 3, 4, 5 veya 6 sütun seçilir; kısa ve dar pencerede kontrollerin görünmesi için dört küçük sütun kullanılır. Başlık ve kaynak/boyut kapağın altındadır; uzun başlık kart genişliğini değiştirmez ve kesilerek gösterilir. Kurulum kullanılamıyorsa kapakta durum etiketi görünür ve başlatma kapalıdır. Kartın kapağı veya adı ayrıntıyı açar; küçük oynatma kontrolü oyunu başlatır. Favori kontrolü hover/odakta görünür; ayrıntıda sürekli erişilebilir.

100 kayıtlık sayfa ve sanallaştırılmış satırlar korunur. Sütun sayısı değişince görünüm modeli yeni sütun sayısına göre gruplanır; filtre ve sayfa konumu korunur. Sayfalama yalnızca birden fazla sayfa varsa görünür. Boş favoriler ve boş aramalar farklı açıklama ve eylem içerir.

## Oyun ayrıntısı

Kapak, ad, platform, kurulum ve Oyna ilk alandadır. Genel bakışta üç ayrı HowLongToBeat kategorisi, gerçek eşleşme adı/kaynak, notlar ve oturumlar gösterilir. Süre bulunmuyorsa Veri yok; henüz alınmadıysa çizgi kullanılır. Oturumlar göreli zaman ve süreyle sunulur; Unix timestamp ve PID kullanıcı akışından çıkarılmıştır.

Düzenle bölümünde başlık/not kaydı, dosya/URL/SteamGridDB kapağı, koleksiyon üyeliği, HowLongToBeat eşleşmesi ve izlenecek executable vardır. Ayrıntıdan dönüş önceki kütüphane/favoriler görünümünü korur. Favori değişimi başlık/not düzenlemelerini yeniden yüklemez; ağdan gelen süreler yalnızca süre alanlarını günceller.

## Discord ve ayarlar

Discord sayfası bağlantı durumu, kullanılabilen ses kontrolleri, arkadaş satırları ve ses odalarını gösterir. Bir arkadaş görüşmeyi Discord'da açar; ses odası ilgili katılma komutunu kullanır. İstemcinin desteklemediği yayın işlemleri etkin gösterilmez. Vencord kurulum/onarım/kaldırma ve elle ID alanı Kurulum ayarları altında açılır.

Ayarlar uygulama güncellemeleri, Vencord otomatik güncellemesi ve kapak sağlayıcısı olarak gruplanır. Kullanıcı hesap bağlantısını erteledi; arayüz bu tercihten bağımsız kullanılabilir. Kaynak tarama bildirimleri Discord'a taşınmaz.

## Klavye ve doğrulama

Kontroller native odak, erişilebilir etiket ve Enter/Space eylemi içerir. Ctrl+F aramaya odaklanır; kartta Enter ayrıntıyı, P başlatmayı ve F favoriyi çalıştırır. Escape ayrıntıdan geri döner. Canlı klavye/ekran okuyucu ve Windows DPI geçişi kabulü ayrıca bekliyor; görsel ölçek denemesi bu kabulün yerine geçmez.

Görsel kontroller gerçek yerel oyunların ayrı doğrulama profilinde yapılır; hesap anahtarı veya Discord kişi bilgisi örnek görsellerde yayımlanmaz. `--screenshot`, `--window-width`, `--window-height`, `--section`, `--detail-tab` ve `--query` ve `--skip-initial-scan` geliştirme kontrolleridir. Görüntüleme, kütüphane yüklemesinin bitmesini bekler ve 30 saniyede zaman aşımına uğrar. Ekran görüntüsü yanındaki `.layout.json` gerçek pencere boyutu, ölçek, sütun ve kayıt sayısını yazar. 10.000 kayıtlık fixture gerçek kullanıcı kütüphanesine karıştırılmaz.

Önceki RAM/CPU ölçümleri eski derlemeye aittir. Yeni tasarımın performansı ayrıca ölçülmeden aynı sonuçları sağladığı iddia edilmez. Önceki sekiz saat testinin raporu tamamlanmadı; sonraki kontrolde süreci çalışmıyordu.

## Native yenileme doğrulaması

Yeni release gerçek kullanıcı profiliyle açıldı; Discord bağlantısı yeniden kuruldu ve 32 arkadaş satırı okundu. Görüntüleme anında ses odası yoktu; uygulama oda uydurmaz. Mikrofon/ses kontrollerinin yetenekleri okundu; ses/DM/kanal eylemi bu görsel kontrolde tetiklenmedi.

Gerçek profilde 12 yerel kaydın 12 kapağı dosya olarak doğrulandı (Steam 8, Epic 1, Riot 3); 10 kayıt kurulu. Kullanıcı hesabı eşitlenmedi. Baldur's Gate 3 ve VALORANT süreleri gerçek profile de kaydedildi. 10.000 kayıtlık ayrı fixture, 100 kaydı 20 satırda yükledi. Normal/dar pencere, oyun ayrıntısı, boş arama, ayarlar, Discord ve Slint 1,5 ölçeği release binary ile görüntülendi. Bunlar yeni RAM/CPU veya uzun kararlılık ölçümü sayılmaz.

Kaynak commit, binary SHA-256, gerçek ekran boyutları ve kabul sınırları [doğrulama kaydında](../benchmarks/windows-native-metadata-and-ui-20261012.json).
