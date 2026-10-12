# Geliştirme sırası

## Aşama 0 — Teknik doğrulama

İlk kodlama işi. Üretim arayüzünden önce iki kritik belirsizlik çözülecek.

| İş | Çıkış kriteri |
| --- | --- |
| Minimal Tauri + React release derlemesi | Pencere, tepsi, yeniden açma ve ikinci örnek davranışı çalışır |
| SQLite + gerçek Steam manifest denemesi | Bir kütüphane okunur ve kaynak AppID ile başlatılır |
| 1.000/10.000 kayıt ve kapak grid'i | RAM, CPU, açılış ve kaydırma ölçümleri raporlanır |
| Social SDK uygulama/erişim denemesi | Arkadaş/presence/DM erişimi ve launcher uygunluğu ayrı sonuçlanır |
| Yayın kontrolü keşfi | Başlat/durdur, oyun/pencere, kalite/çözünürlük, FPS ve ses işlemlerinin her biri destekli, koşullu veya erişilemez olarak kaydedilir |

Discord uygulaması/SDK erişimi yoksa sonuç engel olarak kaydedilir; oyun kütüphanesi denemesi devam eder. Teknik deneme için gerekli hesap erişimi sonraki uygulama aşamasına aittir; bu planı tamamlamak için gerekmiyor.

Karar kapısı: Tauri kaynak hedefini karşılıyor mu? Discord için hangi özellikler resmi, hangileri onaya bağlı, hangileri deneysel? Karşılanmayan hedef çözülmeden kapsam büyütülmez.

12 Ekim 2026: Tauri pencere CPU bütçesini geçince native Slint karşılaştırması yapıldı. Native deneme hedefleri karşıladı; ana arayüz Rust/Slint'e taşındı. Dağıtım kullanıcı isteğiyle ertelendi. Öncelik sırası RAM/CPU ve kararlılık, diğer oyun kaynakları, ardından arayüz ve native Discord entegrasyonu. Tam native sürümün kapaklı/kapaksız ve tepsi ölçümleri ayrı doğrulanır.

## Aşama 1 — Kullanılabilir yerel kütüphane

Steam, Epic ve Riot adaptörleri; SQLite, kaynak bazlı hata izolasyonu, kurulum seçimi, başlatma, favoriler, koleksiyonlar ve elle executable ekleme uygulandı. Kullanıcı düzeltmeleri taramalarda korunur. Windows uninstall/kısayol adaylarından otomatik keşif sonraki iş olarak duruyor.

Çıkış kriteri: gerçek makinedeki bu kaynaklar taranır, yeniden taramada kayıtlar çoğalmaz, kullanıcı düzeltmeleri korunur, eksik launcher uygulamayı bozmaz. Ürün kapsamı bu aşamada Steam/Epic/Riot ve bağımsız oyunlarla sınırlı olarak açıkça belirtilir.

## Aşama 2 — Görseller ve keşif kapsamı

Yerel görseller, SteamGridDB, isteğe bağlı IGDB, kullanıcı kapak değişimi ve önbellek sınırları. GOG, Xbox/Microsoft Store, EA, Ubisoft ve Battle.net adaptörleri doğrulanmış fixture'larla sırayla eklenir. Seçili klasörlerde iptal edilebilir derin tarama gelir.

Çıkış kriteri: internet yokken kütüphane kullanılabilir; yanlış görsel düzeltmesi korunur; indirme/decode sınırları uygulanır. Desteklenmeyen kaynakların kullanıcıya nasıl ekleneceği belli olur.

## Aşama 3 — Discord sosyal özellikleri

Developer Portal uygulaması olmadığı için mevcut çalışma kullanıcının onayladığı yerel Vencord köprüsüyle ilerledi. Arkadaş durumu/aktivitesi ve ses odası doluluğu gerçek Discord oturumunda okundu. DM görüşmesi açma mevcut; mesaj gönderimi eklenmedi. Social SDK/bot erişimi ve genel dağıtım ayrı gelecek işidir.

Çıkış kriteri: her özellik yetenek tablosuna uygun davranır; erişilemeyen odalar veya eski durumlar canlı veri gibi gösterilmez. Kullanıcı adına ve bot adına gönderim ayrımı açıktır.

## Aşama 4 — İstemci ve yayın modülü

Resmi Vencord installer ile kurulum/onarım/kaldırma ve doğrulanabilen yayın kontrolleri. Kullanıcı kapsamı başlat/durdur, oyun/pencere değiştirme, kalite/çözünürlük, FPS ve ses ayarlarının tamamını içerir. İstemci sürüm uyumu, geri dönüş ve köprü bağlantı güvenliği bu modülün parçasıdır. Yayın kontrolleri yalnızca teknik denemede çalışan işlemler için eklenir.

Çıkış kriteri: desteklenmeyen Discord sürümünde modül kendi kendini kapatır; kütüphane işlevleri devam eder. Kurulum veya onarım işlemi sonucunun doğrulanması olmadan başarılı bildirimi verilmez.

## Aşama 5 — Eklentiler ve dağıtım

Sürümlü eklenti protokolü, ayrı süreç yaşam döngüsü, yetki tanımları ve uyumluluk kontrolü. İlk eklenti yeni bir oyun kaynağı veya metadata sağlayıcısı olabilir. Ardından Windows installer, imzalı güncelleme, migration yedeği ve geri dönüş yolu.

Çıkış kriteri: bozuk eklenti ana uygulamayı kapatmaz; güvenilmeyen eklentiler için sandbox kararı olmadan genel eklenti mağazası açılmaz. Güncelleme zinciri ve veri geçişi gerçek release paketinde doğrulanır.

## Kesinleşen kapsam

- İlk sürüm Windows; diğer işletim sistemleri sonraki aşamalarda değerlendirilecek. Windows 11 x64 geliştirme referansı; Windows 10 uyumluluğu ayrıca doğrulanacak.
- Discord yayını için başlatma/durdurma, paylaşılan oyun/pencereyi değiştirme, kalite/çözünürlük, FPS ve ses ayarlarının tamamı hedefleniyor. Kapsam seçimi tamamlandı; her işlemin erişimi ve kararlılığı Aşama 0'da doğrulanacak.

## Açık kararlar

| Karar | Mevcut varsayım | Ne zaman kesinleşir? |
| --- | --- | --- |
| Kesin RAM tavanı | Açık pencere 180 MiB, tepsi 40 MiB hedef | Kullanıcı beklentisi ve Aşama 0 ölçümü |
| Yayın kontrollerinin teknik desteği | Her işlem ayrı doğrulanacak | Aşama 0 denemesi ve istemci sürüm uyumu |
| Discord SDK launcher uygunluğu | Doğrulanmadı | Uygulama erişimi ve gerekirse Discord incelemesi |
| Kişisel kullanım / genel dağıtım | İlk deneme kişisel | Bot/metadata servisi ve dağıtım öncesi |
| Ürün adı | Game Library | Görsel tasarım aşaması |

Sonraki işler: gerçek Epic/Riot kabulü, sekiz saat kararlılık sonucunun değerlendirilmesi, oyun frame-time/GPU ölçümü, Discord sürümünde yayın yeteneklerinin araştırılması ve ilk imzalı release zincirinin doğrulanması. Güncel kabul durumları [devam planında](devam-plani.md) tutulur; minimal masaüstü denemesi tamamlandı.
