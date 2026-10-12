# İlk Windows sürümü

## Mevcut özellikler

- Registry ve standart kurulum yollarından Steam keşfi.
- Modern ve eski `libraryfolders.vdf` biçimleri, birden fazla Steam kütüphanesi.
- Steam/SteamLibrary veya `steamapps` klasörü seçerek ek kaynak tanımlama.
- Sınırlı dosya okuma, manifest ayrıştırma ve kurulum kontrolü.
- SQLite kalıcılığı, arama, favoriler, kurulu filtresi ve 100 kayıtlık sayfalar.
- Native Rust/Slint arayüzü, sanallaştırılmış kapak satırları ve varsa Steam'in yerel kapak dosyaları.
- Ayrı işçi thread'inde 180×270 thumbnail hazırlama; 8 MiB dosya, 4096×4096 boyut ve 64 MiB decode tahsis sınırı.
- Kayıtlı ve erişilebilir kurulumun sayısal AppID'siyle `steam://rungameid/` başlatma.
- Kütüphane pencere/tepsi yaşam döngüsü ve tek uygulama örneği.
- Özel çalışma kümesi, toplam çalışma kümesi, private bytes ve makine toplamına göre CPU.
- Süreç ayrıntıları ve 60 saniyelik yerel JSON ölçüm raporu.

Bu sürümde Steam hesabına giriş yapılmaz, internet metadata isteği gönderilmez ve bütün disk taranmaz. İlk açılışta tarama yapılır; sonraki açılışlarda SQLite verisi yüklenir. Değişen kurulumlar Yeniden tara ile güncellenir. Manifest değişikliklerini sürekli izleme sonraki aşamaya bırakıldı.

## Kullanım

`target/release/game-library-native.exe` dosyasını çalıştır. İlk açılışta Steam'in kurulum kaydı varsa kütüphane otomatik taranır. Bulunamazsa Steam klasörü ekle ile kurulumunu veya ek SteamLibrary klasörünü seç. Oyun kartındaki Oyna başlatma isteğini Steam'e iletir; Steam oturum açma veya başka bir uyarı gösterirse onun ekranında devam edilir.

Başlatma isteğinin Windows tarafından kabul edilmesi oyunun açıldığının kanıtı değildir. İlk sürüm bunu arayüzde başlatma isteği olarak bildirir; gerçek oyun süreç/oturum takibi henüz yok.

Pencere kapatılınca native pencere gizlenir. Tepsi simgesi ve Rust çekirdek açık kalır. Uygulama veya tepsi menüsünden Çıkış uygulamayı tamamen kapatır; simgeye sol tıklamak pencereyi yeniden açar. İkinci örnek Windows event'iyle mevcut pencereyi öne getirip kapanır. WebView ve tarayıcı süreci kullanılmaz.

## Ölçüm

Performans ekranındaki Anlık ölçüm tek örnek alır. 60 saniyelik ölçüm sırasında örnekler saniyede bir kaydedilir. Ölçüm kapalıyken sürekli kaynak örneklemesi yok. Ölçüm başlatıldıktan sonra pencereyi kapatarak tepsi tüketimi de izlenebilir; karma senaryo açık/tepsi ortalaması olur. Tepside ölçüm sürerken arayüz güncellenmez.

Karşılaştırılabilir ayrı senaryolar için `scripts/measure.ps1` kullanılır. Her senaryo 8 saniye ısınma sonrasında 60 saniye ölçülür, JSON raporu `artifacts/measurements/` altına kaydedilir. Pencere senaryosunda Performans ekranına geçmeden kütüphaneyi boşta bırak.

Özel çalışma kümesi uygulamaya özel fiziksel RAM'dir. Private bytes özel commit'i gösterir ve fiziksel RAM ile aynı değildir. Toplam çalışma kümesi paylaşılan sayfaları süreçler arasında tekrar sayabilir. Windows genişletilmiş RAM sayacını desteklemezse özel RAM bilinmiyor gösterilir. Erişilemeyen süreç varsa sonuç kısmi işaretlenir.

CPU bütün mantıksal işlemcilere bölünerek raporlanır. Süreç üyeliği 5 saniyede bir yenilenir; bilinen bir süreç kaybolursa hemen yeniden okunur. RAM/CPU sayaçları her örnekte okunur. Bir örnek aralığında başlayıp biten veya ölçüm dışı parent'a taşınan süreçler tam yakalanmayabilir. Native uygulama tek süreçtir; önceki Tauri karşılaştırmasında aynı sayaç Rust ana PID'si ve WebView2 çocuklarını da toplar. Kısa ömürlü yardımcı süreçlerin tam CPU hesabı ve GPU/VRAM ölçümü için daha sonra ETW/WPR doğrulaması gerekir.

## Fixture ve yerel kabul testi

`fixtures/steam/` örnekleri gerçek oyun kurulumu veya kullanıcının envanteri değildir. Testler modern/eski kütüphane biçimi, birden fazla kök, bozuk manifest, yanlış AppID, güvensiz kurulum yolu, eksik kurulum, iptal, yeniden tarama, favori korunması, arama ve sayfalama içerir.

Yoğunluk denemesi için ayrı veri klasöründe örnek kütüphane oluşturulabilir:

```powershell
pwsh -File scripts/create-benchmark-library.ps1 -Count 1000
target/release/game-library-native.exe --scan-json --steam-root artifacts/steam-fixture-1000 --data-dir artifacts/fixture-data --output artifacts/fixture-scan.json
pwsh -File scripts/measure.ps1 -Mode Window -DataDirectory artifacts/fixture-data
```

Fixture Game kayıtları yalnızca test verisidir; Oyna ile başlatılmamalıdır. Test verisi ayrı `--data-dir` altında tutulur, normal kullanıcı kütüphanesine eklenmez.

Kapaklı veri için üretim komutuna `-WithArtwork` eklenir ve farklı bir OutputDirectory seçilir. 50 pencere/tepsi döngüsü için `game-library-native.exe --lifecycle-test artifacts/native-lifecycle.json --data-dir artifacts/fixture-data` kullanılır. Bu deneme her durumda kısa beklemeden sonra RAM/süreç sayısını kaydeder; açılış gecikmesinin veya uzun süreli kararlılığın ölçümü değildir.

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
| Native tam sürüm, kapaksız | 12,15 MiB | 12,33 MiB | 13,34 MiB | %0,141 |
| Native tam sürüm, kapaklı | 30,80 MiB | 30,86 MiB | 32,02 MiB | %0,110 |
| Native doğrudan tepsi, kapaklı | 23,29 MiB | 23,31 MiB | 24,55 MiB | %0,013 |

Tauri RAM ve tepsi bütçesini karşıladı; açık pencere CPU hedefini karşılamadı. Native deneme ve tam sürüm RAM/CPU hedeflerini karşıladı. Ana arayüz Rust/Slint'e taşındı. `apps/native-probe` aynı SQLite verisini ve kaynak ölçüm modülünü kullanan küçük karşılaştırma uygulamasıdır; tam Steam/tepsi/ölçüm sürümü `apps/native` içinde.

Native sonucu `window-20261012-114743.json` raporundan, 61 örnek ve tek süreçten alındı; kısmi ölçüm yok. Önceki native deneme derleme sırasında çalıştırıldığı ve arama alanı odaklandığı için karar ölçümü olarak kullanılmadı. Bu karşılaştırma kapaksız ve boşta pencereyi kapsıyor; görseller, etkileşim ve GPU etkisi için ek ölçüm gerekiyor.

Tam native sürüm `c3cd01a` koduyla, derleme işi çalışmadan tekrar ölçüldü. Kapaklı veri setinde 1.000 JPEG kapak ve 100 kayıtlık görünüm modeli kullanıldı. Her ayrı boşta senaryoda 61 örnek, tek süreç ve eksiksiz sayaçlar kaydedildi. Toplam çalışma kümesi kapaksız pencerede 36,88 MiB, kapaklı pencerede 54,99 MiB; bu değerler özel RAM'e ek olarak paylaşılan sistem sayfalarını da içerir. Kaynak raporları, executable SHA-256, makine ve kapsam özeti [benchmark kaydında](../benchmarks/windows-native-20261012.json).

Kapaklı native pencerede 50 aç/kapat döngüsü tamamlandı. Tepside özel RAM ilk döngüde 30,87 MiB, son döngüde 31,33 MiB; her örnekte bir süreç, sıfır kısmi ölçüm. Bu küçük artış tek başına uzun süreli bellek sızıntısı olmadığı sonucunu vermez; 8 saatlik test ayrı kalır. Sayfalama, 1.000. kayıt araması, favori kaydı, ölçüm başlatma/iptal, kapak gösterimi, tepsi dönüşü ve ikinci örneğin mevcut pencereyi açıp 0 koduyla kapanması Windows arayüzünde doğrulandı. 15 Rust testi ve çalışma alanı Clippy denetimi geçti.

Beş pencere/tepsi döngüsünde WebView2 süreçleri sona erdi, tepside bir Rust süreç kaldı ve özel RAM 5,47–6,43 MiB aralığındaydı. Doğrudan tepsi açılışı ile pencereyi kullanıp kapatma senaryosu aynı değildir. Windows arayüzünde favori ekleme, 1.000. kaydı arama, ölçüm başlatma/iptal ve tek uygulama örneği kontrolleri yapıldı. Parser/veri testleri ve kaynak ölçümü testleri geçti.

Dağıtım ertelendi. Geliştirme sırası kullanıcı tarafından RAM/CPU ve kararlılık, diğer oyun kaynakları, ardından arayüz ve Discord olarak netleştirildi. Mevcut exe yalnızca geliştirme ve ölçüm için kullanılıyor.

8 saat kararlılık, açılış gecikmesi p95, yoğun kaydırma frame-time, GPU/VRAM ve gerçek oyun üzerindeki etki henüz doğrulanmadı. Bu sonuçlar gerçek Steam kurulumu veya bütün bilgisayarlar için garanti değildir.
