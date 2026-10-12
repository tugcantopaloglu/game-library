# İlk Windows sürümü

## Mevcut özellikler

- Registry ve standart kurulum yollarından Steam keşfi.
- Modern ve eski `libraryfolders.vdf` biçimleri, birden fazla Steam kütüphanesi.
- Steam/SteamLibrary veya `steamapps` klasörü seçerek ek kaynak tanımlama.
- Sınırlı dosya okuma, manifest ayrıştırma ve kurulum kontrolü.
- SQLite kalıcılığı, arama, favoriler, kurulu filtresi ve 200 kayıtlık sayfalar.
- Sanallaştırılmış kapak grid'i ve varsa Steam'in yerel kapak dosyaları.
- Kayıtlı ve erişilebilir kurulumun sayısal AppID'siyle `steam://rungameid/` başlatma.
- Kütüphane pencere/tepsi yaşam döngüsü ve tek uygulama örneği.
- Özel çalışma kümesi, toplam çalışma kümesi, private bytes ve makine toplamına göre CPU.
- Süreç ayrıntıları ve 60 saniyelik yerel JSON ölçüm raporu.

Bu sürümde Steam hesabına giriş yapılmaz, internet metadata isteği gönderilmez ve bütün disk taranmaz. Oyunları yeniden bulmak için açılış taraması veya Yeniden tara kullanılır. Manifest değişikliklerini sürekli izleme sonraki aşamaya bırakıldı.

## Kullanım

Release exe'yi çalıştır. Steam'in kurulum kaydı varsa kütüphane otomatik taranır. Bulunamazsa Steam klasörü ekle ile kurulumunu veya ek SteamLibrary klasörünü seç. Oyun kartındaki Oyna başlatma isteğini Steam'e iletir; Steam oturum açma veya başka bir uyarı gösterirse onun ekranında devam edilir.

Başlatma isteğinin Windows tarafından kabul edilmesi oyunun açıldığının kanıtı değildir. İlk sürüm bunu arayüzde başlatma isteği olarak bildirir; gerçek oyun süreç/oturum takibi henüz yok.

Pencere kapatılınca WebView yok edilir. Tepsi simgesi ve Rust çekirdek açık kalır. Tepsi menüsünden Çıkış uygulamayı tamamen kapatır; simgeye sol tıklamak pencereyi yeniden oluşturur.

## Ölçüm

Performans ekranı açıkken 2 saniyede bir örnek alınır. 60 saniyelik ölçüm sırasında örnekler saniyede bir kaydedilir. Diğer ekranlarda sürekli kaynak örneklemesi yok. Ölçüm başlatıldıktan sonra pencereyi kapatarak tepsi tüketimi de izlenebilir; karma senaryo açık/tepsi ortalaması olur.

Karşılaştırılabilir ayrı senaryolar için `scripts/measure.ps1` kullanılır. Her senaryo 8 saniye ısınma sonrasında 60 saniye ölçülür, JSON raporu `artifacts/measurements/` altına kaydedilir. Pencere senaryosunda Performans ekranına geçmeden kütüphaneyi boşta bırak.

Özel çalışma kümesi uygulamaya özel fiziksel RAM'dir. Private bytes özel commit'i gösterir ve fiziksel RAM ile aynı değildir. Toplam çalışma kümesi paylaşılan sayfaları süreçler arasında tekrar sayabilir. Windows genişletilmiş RAM sayacını desteklemezse özel RAM bilinmiyor gösterilir. Erişilemeyen süreç varsa sonuç kısmi işaretlenir.

CPU bütün mantıksal işlemcilere bölünerek raporlanır. Süreç üyeliği 5 saniyede bir yenilenir; bilinen bir süreç kaybolursa hemen yeniden okunur. RAM/CPU sayaçları her örnekte okunur. Bir örnek aralığında başlayıp biten veya ölçüm dışı parent'a taşınan süreçler tam yakalanmayabilir. Bu ilk sürümde süreç ağacı Rust ana PID'si ve WebView2 çocuklarıyla sınırlıdır. Kısa ömürlü süreçlerin tam CPU hesabı ve GPU/VRAM ölçümü için daha sonra ETW/WPR doğrulaması gerekir.

## Fixture ve yerel kabul testi

`fixtures/steam/` örnekleri gerçek oyun kurulumu veya kullanıcının envanteri değildir. Testler modern/eski kütüphane biçimi, birden fazla kök, bozuk manifest, yanlış AppID, güvensiz kurulum yolu, eksik kurulum, iptal, yeniden tarama, favori korunması, arama ve sayfalama içerir.

Yoğunluk denemesi için ayrı veri klasöründe örnek kütüphane oluşturulabilir:

```powershell
pwsh -File scripts/create-benchmark-library.ps1 -Count 1000
target/release/game-library.exe --scan-json --steam-root artifacts/steam-fixture-1000 --data-dir artifacts/fixture-data --output artifacts/fixture-scan.json
pwsh -File scripts/measure.ps1 -Mode Window -DataDirectory artifacts/fixture-data
```

Fixture Game kayıtları yalnızca test verisidir; Oyna ile başlatılmamalıdır. Test verisi ayrı `--data-dir` altında tutulur, normal kullanıcı kütüphanesine eklenmez.

## Gerçek Steam ortamında kabul

1. Steam kurulumunun ve ek kütüphanelerinin doğru bulunduğunu kontrol et.
2. En az iki oyunun adı, AppID'si ve kurulum durumunu Steam ile karşılaştır.
3. Oyna ile bir oyun başlat; Steam'de gerekirse oturum açıp oyunun gerçekten açıldığını doğrula.
4. Favori ekle, yeniden tara ve uygulamayı tamamen kapatıp aç; tercihin korunduğunu kontrol et.
5. Steam üzerinden bir oyunu kaldır veya taşı, yeniden tara; diğer kayıtların korunmasını doğrula.
6. Aynı release dosyasıyla açık pencere ve tepsi için ayrı 60 saniyelik ölçüm al.

Bu geliştirme ortamında Steam kurulu değil. Gerçek launcher ve oyun başlatma testi sonraki kurulu ortamda yapılacak.

## Release ölçüm sonuçları

Windows 11 Enterprise build 26300, Intel Core i5-1235U, 12 mantıksal işlemci ve 31,64 GiB RAM üzerinde. Veri seti 1.000 örnek Steam kaydı; kapaksız grid. Release uygulaması, 8 saniye ısınma ve 60 saniye ölçüm. Toplamlar Rust ve uygulamaya ait WebView2 süreçlerini içeriyor.

| Senaryo | Ortalama özel RAM | En yüksek özel RAM | Private bytes | Ortalama CPU |
| --- | --- | --- | --- | --- |
| Açık Tauri penceresi | 119,37 MiB | 122,51 MiB | 177,92 MiB | %0,593 |
| Doğrudan tepsi açılışı | 2,74 MiB | 2,76 MiB | 3,48 MiB | %0,013 |
| Native Slint denemesi | 10,84 MiB | 10,93 MiB | 11,70 MiB | %0,123 |

Tauri RAM ve tepsi bütçesini karşıladı; açık pencere CPU hedefini karşılamadı. Native Slint denemesi açık pencere RAM/CPU bütçesini karşıladı. Ana arayüz Rust/Slint'e taşınacak. `apps/native-probe` aynı SQLite verisini ve kaynak ölçüm modülünü kullanır; software renderer, sanallaştırılmış satırlar ve boşta arama alanına odak vermeyen pencere içerir. Steam tarama/başlatma ve tepsi işlevleri bu karşılaştırma uygulamasında yoktur; tam sürüme taşındıktan sonra ölçümler tekrarlanacak.

Native sonucu `window-20261012-114743.json` raporundan, 61 örnek ve tek süreçten alındı; kısmi ölçüm yok. Önceki native deneme derleme sırasında çalıştırıldığı ve arama alanı odaklandığı için karar ölçümü olarak kullanılmadı. Bu karşılaştırma kapaksız ve boşta pencereyi kapsıyor; görseller, etkileşim ve GPU etkisi için ek ölçüm gerekiyor.

Beş pencere/tepsi döngüsünde WebView2 süreçleri sona erdi, tepside bir Rust süreç kaldı ve özel RAM 5,47–6,43 MiB aralığındaydı. Doğrudan tepsi açılışı ile pencereyi kullanıp kapatma senaryosu aynı değildir. Windows arayüzünde favori ekleme, 1.000. kaydı arama, ölçüm başlatma/iptal ve tek uygulama örneği kontrolleri yapıldı. Parser/veri testleri ve kaynak ölçümü testleri geçti.

Dağıtım ertelendi. Geliştirme sırası kullanıcı tarafından RAM/CPU ve kararlılık, diğer oyun kaynakları, ardından arayüz ve Discord olarak netleştirildi. Mevcut exe yalnızca geliştirme ve ölçüm için kullanılıyor.

8 saat kararlılık, 50 pencere döngüsü, GPU/VRAM ve gerçek oyun frame-time etkisi henüz doğrulanmadı. Bu sonuçlar gerçek Steam kurulumu veya bütün bilgisayarlar için garanti değildir.
