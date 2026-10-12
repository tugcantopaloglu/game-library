# Epic ve Riot desteği

12 Ekim 2026: Steam, Epic ve Riot aynı native Rust/Slint uygulamasında taranır. Kaynak işi tek, 16 komutla sınırlı işçi kuyruğunda yürür. Arka planda sürekli tarama, ağ isteği veya launcher izleme timer'ı eklenmedi.

## Epic Games

Standart kaynak `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests` altındaki `.item` dosyalarıdır. Epic Games kaynak ekranından bu dizin veya onu içeren Epic veri klasörü seçilebilir. Oyun kimliği namespace/catalog/AppName alanlarından alınır. Kurulum dizini ve kurulum içindeki `LaunchExecutable` doğrulanır. Yarım kurulum oynanabilir sayılmaz; Unreal Engine, eklenti ve DLC kayıtları ayrı oyun olarak içe aktarılmaz.

Başlatma `com.epicgames.launcher://apps/<namespace>%3A<catalog>%3A<AppName>?action=launch&silent=true` üzerinden Epic Games Launcher'a iletilir. Eski, yalnızca AppName içeren manifestler için belgelenmiş eski URI biçimi desteklenir. Manifestteki `LaunchCommand` shell komutu olarak çalıştırılmaz. Epic hesabı, DRM ve oyunun ihtiyaç duyduğu diğer istemciler Epic tarafından yönetilir. [Epic protokol belgesi](https://assets-unreal2-epic-prod-us2.s3.dualstack.us-east-1.amazonaws.com/original/4X/f/c/4/fc4342c50e1a0abf34943e019ee90bacccc62950.pdf), [manifest alanları](https://github.com/JosefNemec/PlayniteExtensions/blob/master/source/Libraries/EpicLibrary/Models/LauncherInstalled.cs).

## Riot Games

`%PROGRAMDATA%\Riot Games` altında `RiotClientInstalls.json` ve `Metadata/<ürün>.<patchline>/<ürün>.<patchline>.product_settings.yaml` okunur. Windows'un HKCU/HKLM ve 32/64 bit uninstall kayıtları ikinci keşif yoludur. Riot istemcisi associated_client/default/live/beta yolları, kayıtlı riotclient protokolü ve standart kurulumdan bulunur. Bu veri klasörü veya Metadata klasörü kaynak ekranından elle seçilebilir.

Desteklenen ürün kimlikleri `league_of_legends`, `valorant`, `bacon` ve `lion`: League of Legends, VALORANT, Legends of Runeterra ve 2XKO. Farklı patchline kurulumları ayrı kayıttır; TFT ayrı executable olarak eklenmez, League istemcisi üzerinden erişilir. [Kurulum kayıtlarını kullanan kaynak uygulama](https://github.com/ASchoe311/RiotGamesLibrary/blob/master/RiotClient.cs), [ürün başlatma argümanları](https://github.com/ASchoe311/RiotGamesLibrary/blob/master/RiotGamesLibrary.cs).

Riot oyun executable'ı doğrudan çalıştırılmaz. `RiotClientServices.exe`, ayrı `--launch-product=<ürün>` ve `--launch-patchline=<patchline>` argümanlarıyla başlatılır. Working directory istemcinin klasörüdür. Ürün allowlist'i, kimlik karakter sınırı, kurulum içindeki executable ve istemci dosya adı doğrulanır. İstemci yoksa başlatma hatası gösterilir. Kimlik doğrulama ve anti-cheat Riot Client'ta kalır.

## Kalıcılık ve hata davranışı

SQLite şeması 1'den 2'ye geçmeden önce aynı veri klasöründe `library.schema-1-backup-<zaman>.db` oluşturulur. Steam kimlikleri ve favorileri korunur. Kaynak kökleri kaynak türüyle ayrılır; Epic yeniden taraması Riot/Steam kayıtlarını değiştiremez. Eksik disk veya bozuk manifest tam snapshot sayılmaz. Favoriler silinmez; tamamlanmış kaynak taramasında kaybolan kurulum erişilemiyor olur.

Her kayıt en fazla 1 MiB okunur; dizin başına 20.000 kayıt, YAML'da 16 derinlik ve 10.000 olay sınırı vardır. YAML alias/anchor ve özel tag'ler kabul edilmez. Hatalar 100 uyarıyla sınırlanır. Windows extended-path gösteriminin değişmesi Riot kurulum kimliğini değiştirmez.

## Doğrulama

`fixtures/epic` ve `fixtures/riot` gerçek kurulum değildir. `scripts/create-source-fixtures.ps1` ayrı bir artifacts dizininde Epic manifestleri, dört Riot metadata kaydı ve yalnızca argümanları dosyaya yazan bir test istemcisi üretir. Oyun executable dosyaları çalıştırılabilir oyun içermez. Normal kullanıcı kütüphanesine bu veriler eklenmez.

`--steam-root`, `--epic-root`, `--riot-root` kaynak ekler. `--scan-json` tüm kaynakları tarar; `--list-json --source epic|riot|steam` mevcut veriyi sorgular. `--launch-plan <kayıt-id> --output <dosya>` başlatma tanımını çalıştırmadan doğrular. Gerçek Epic/Riot oturum, oyun ve anti-cheat davranışı kurulu oyun ortamında ayrıca kabul testine alınır.

Örnek veri üretmek için Rust toolchain'i gerekir. Var olan çıktı klasörü üzerine yazılmaz:

```powershell
pwsh -File scripts/create-source-fixtures.ps1 -EpicCount 1000
```

Native arayüzde kaynak sayıları, Riot ürünleri, kaynak filtresi ve kaynak ekranından kütüphaneye dönüş doğrulandı. VALORANT'ın Oyna düğmesi mock Riot istemcisini kendi klasöründe, tam olarak `--launch-product=valorant` ve `--launch-patchline=live` argümanlarıyla çalıştırdı. Epic başlatma URI'si çalıştırmadan doğrulandı. Bu testler gerçek oyun açılışı veya hesap bağlantısı sayılmaz.

`cargo test --workspace` 26 testi geçti. Şema geçişi ve yedeği, Steam regresyonları, kaynakların birbirini değiştirmemesi, eksik disk sonrası Riot kimliği/favorisi, bozuk JSON/YAML, iptal, başlatma yolları ve ölçüm kesintileri kapsandı. `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` ve karşılaştırma uygulamasının `pnpm webview:check` denetimi geçti.

## Kaynak tüketimi

Windows 11 Enterprise 26300, i5-1235U, 12 mantıksal işlemci ve 31,64 GiB RAM üzerinde release derlemesi ölçüldü. Veri seti 1.000 Steam, 1.000 Epic ve dört Riot kaydıdır. İlk 100 kayıtlık sayfada 99 küçültülmüş Steam kapağı görünüm modeline yüklendi. 8 saniye ısınma sonrası 60 saniye boşta ölçümleri:

| Senaryo | Ortalama özel RAM | En yüksek özel RAM | Private bytes | Ortalama CPU |
| --- | --- | --- | --- | --- |
| Pencere açık | 32,32 MiB | 32,38 MiB | 33,58 MiB | %0,199 |
| Doğrudan tepsi açılışı | 24,06 MiB | 24,12 MiB | 25,28 MiB | %0,028 |

İki ölçüm de tek süreçli, eksiksiz ve kesintisizdir; gerçek süreleri 60,17 ve 60,24 saniye, en uzun örnek aralıkları 1,018 ve 1,020 saniyedir. CPU yüzdesi makine toplamına göre hesaplanır. RAM/CPU bütçeleri karşılandı. Tam kaynak taraması 2.004 kurulum buldu, uyarısız 487,41 ms sürdü; sistemdeki üç Epic engine/eklenti manifesti oyun olarak sayılmadı.

20,5 ve 51,5 dakikalık örnek boşlukları içeren iki pencere kaydı karşılaştırmadan çıkarıldı. Ölçüm kodu bu tür kesintileri işaretler; bunların nedeni doğrulanmadı. GPU/VRAM, oyun içi frame-time ve sekiz saatlik stabilite bu aşamada ölçülmedi. [Commit, binary hash ve ölçüm özeti](../benchmarks/windows-native-sources-20261012.json).
