# Oyun keşfi ve görseller

## Kapsam

Amaç launcher'dan bağımsız bir kütüphane. Steam, Epic ve Riot ilk adaptörler; GOG, Xbox/Microsoft Store, EA, Ubisoft, Battle.net, bağımsız oyunlar ve elle eklenen oyunlar sonraki adaptörlerdir.

Bilgisayardaki her `.exe` dosyasını oyun saymak doğru sonuç vermez. Taşınabilir oyunlar, özel kurulumlar, çıkarılmış arşivler ve erişilemeyen diskler otomatik olarak kesin tespit edilemeyebilir. Bu yüzden otomatik keşfe seçili klasör taraması, kısayol içe aktarma ve elle ekleme eşlik eder.

## Kaynak adaptörleri

Steam, Epic ve Riot adaptörleri uygulandı. Epic/Riot'un çalışan kaynak yolları, başlatma tanımları ve kabul sınırları [ayrı belgede](epic-riot.md). Diğer kaynaklar plan kapsamındadır.

Aşağıdaki yollar aday keşif noktalarıdır. Launcher sürümleri iç formatlarını değiştirebilir; gerçek örnek dosyalarla doğrulama yapılacak. Sabit bir `C:` yolu tek kaynak kabul edilmeyecek.

| Kaynak | Keşif | Başlatma |
| --- | --- | --- |
| Steam | HKCU/HKLM kurulum kayıtları, `libraryfolders.vdf`, her kütüphanedeki `appmanifest_*.acf` | AppID ile Steam protokolü |
| Epic | ProgramData manifestleri ve kurulum kayıtları; `.item` kimlikleri | Doğrulanmış Epic launcher URI'si |
| Riot | Registry, `RiotClientInstalls.json`, ürün metadata'sı ve kısayollar | Riot Client ürün/patchline argümanları |
| GOG | Kurulum registry kayıtları ve yerel manifestler | Galaxy veya doğrulanmış exe |
| Xbox / Microsoft Store | Kullanıcıya ait paket kaydı ve uygulama kimliği | AUMID ile işletim sistemi başlatma |
| EA / Ubisoft / Battle.net | Kaynak bazlı kurulum kayıtları, manifestler ve kısayollar | Launcher'a özgü doğrulanmış komut |
| Bağımsız oyun | Kullanıcı klasörü, Windows uninstall kayıtları, Start Menu/Desktop kısayolları | Kullanıcının onayladığı exe ve argüman dizisi |

Microsoft Store oyunlarını keşfetmek için `WindowsApps` klasörünün sahipliği değiştirilmeyecek. Registry'nin 32/64 bit görünümleri, birden fazla Steam kütüphanesi ve kaldırılmış launcher kalıntıları dikkate alınacak.

Bu bilgisayarda planlama sırasında varsayılan Epic manifest klasörünün varlığı doğrulandı; varsayılan Riot aday yolları bulunmadı. Bu yalnızca yol kontrolüdür, oyun envanteri veya Riot'un kurulu olmadığı sonucu değildir.

## Tarama akışı

1. Bilinen kaynak köklerini registry, launcher kayıtları ve kullanıcı tercihlerinden bul.
2. Küçük manifest dosyalarını sınırlandırılmış okuyucularla ayrıştır.
3. Kaynak kimliğini ve kurulum yolunu doğrula; kurulu, eksik, erişilemiyor durumlarını ayır.
4. Kaynak bazlı geçici sonucu hazırla ve tamamlanan kaynak için kısa veritabanı işlemiyle uygula.
5. Oyun listesini hemen göster; metadata ve kapak işlerini sonradan sıraya koy.
6. İlgili manifest klasörlerinde değişiklikleri izle; yalnızca değişen kaynağı yeniden tara.

Bir kaynağın hata vermesi diğer kaynakların sonuçlarını engellemez. İptal edilen veya yarım kalan tarama kurulu oyunları kaldırılmış olarak işaretlemez. Çıkarılabilir disk erişilemiyorsa oyun kaydı korunur. Tamamlanmış bir kaynak taraması bile kullanıcı notlarını silmez.

Manuel derin tarama seçili köklerde, görünür ilerleme ve iptal ile çalışacak. Junction/symlink döngülerine, ağ sürücülerine, erişim hatalarına, dosya boyutuna ve derinliğe sınır konacak. Sürücülerin tamamında sürekli arka plan taraması yapılmayacak.

## Eşleştirme

Bir manifest sonucu kaynak kimliğiyle kesinleşir. Kısayol veya klasör adıyla bulunan adayın güveni daha düşüktür; kullanıcı düzeltmesi gerekebilir. Aynı adlı launcher, uninstaller ve anti-cheat uygulamaları ayrı oyun olarak gösterilmeyecek.

Yalnızca başlık benzerliğiyle farklı mağaza sürümleri birleştirilmeyecek. Eşleştirme sırası: kaynak oyun kimliği, doğrulanmış dış kimlik, yayıncı/sürüm bilgisi, kullanıcı seçimi. Kullanıcı düzeltmesi sonraki taramada korunacak.

Başlatma, manifestten gelen metni shell komutu olarak çalıştırmayacak. Protokol/launcher allowlist'i, exe yolu ve ayrı argüman dizisi kullanılacak. Oyunla birlikte açılan launcher sürecinin sona ermesi otomatik olarak oyunun kapandığı anlamına gelmez; oturum takibi doğrulanmış oyun sürecini veya kaynak sinyalini kullanacak.

## Kapak ve metadata

Öncelik: kullanıcı kapak seçimi, yerel launcher görselleri, doğrulanmış sağlayıcı eşleşmesi, SteamGridDB, isteğe bağlı IGDB, yerel ikon/başlık yedeği. Her kaynak bütün kapak türlerini sunmayabilir.

SteamGridDB API erişimi kullanıcının API anahtarı ve sağlayıcı koşullarıyla doğrulanacak. Otomatik görsel indirme için izin verilen kaynak adresleri sınırlanacak. [SteamGridDB API](https://www.steamgriddb.com/api/v2).

IGDB Twitch kimlik doğrulamasıyla çalışıyor; paylaşılan bir client secret masaüstü ikilisine konmayacak. İlk seçenek kullanıcının kendi kimlik bilgileri; genel dağıtım için küçük bir metadata servisi ayrı karar. API'nin belgelenmiş sınırı 4 istek/saniye ve 8 eşzamanlı istektir; bizim kuyruk başlangıçta 2 indirme ve en fazla 2 istek/saniye kullanacak. Ticari kullanım koşulları dağıtım kapsamına göre doğrulanacak. [IGDB API belgeleri](https://api-docs.igdb.com/).

## Önbellek

- Grid için yaklaşık 320 × 480, ayrıntı ekranı için en fazla 1280 px varyant; özgün çözünürlük sürekli bellekte tutulmaz.
- Görünür ve görünmek üzere olan satırlar yüklenir; sanallaştırma görünmeyen kartları kaldırır.
- İndirilen dosya türü, sıkıştırılmış boyut ve açılmış piksel boyutu kontrol edilir. Başlangıç sınırı 8 MiB indirme ve 16 megapiksel decode.
- Dosyalar geçici adla yazılıp atomik olarak yerine taşınır; kesilen indirme eski kapağı bozmaz.
- Önbellek varsayılan 512 MiB disk bütçesi ve son erişime göre tahliye kullanır; kullanıcı seçili kapakları ayrı tutulur.
- Uzak sağlayıcı çalışmıyorsa yerel görsel gösterilir. `Retry-After`, gecikmeli tekrar ve başarısız eşleşme önbelleği gereksiz istekleri engeller.
- Görsel decode kuyruğu tek worker ile başlar; büyütülmesi ölçüme bağlıdır.

Arama, başlatma ve tarama metadata API'sine bağlı olmayacak. İnternet kesildiğinde kurulu oyun kütüphanesi çalışmaya devam edecek.
