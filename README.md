# Game Library

Bilgisayardaki oyunları tek yerde toplayan, yerel verilerle çalışan masaüstü kütüphanesi. Oyun bulma, başlatma, kapak yönetimi ve Discord bağlantısı ayrı modüller üzerinden çalışır.

Öncelik sırası: kararlılık, düşük kaynak tüketimi, doğru oyun tespiti, kullanım kolaylığı ve görsel kalite.

## Proje durumu

12 Ekim 2026: native Windows sürümü Steam/Epic/Riot keşfi ve başlatma, elle oyun ekleme, koleksiyonlar, kullanıcı başlık/not/kapak düzeltmeleri, süreçle doğrulanan oturumlar ve yerel Vencord bağlantısı içeriyor. Beholder: Conductor gerçek Steam başlatma testi geçti; Discord Stable bağlantısında arkadaş ve ses odası verisi okundu. Yayın kontrolleri bu istemcide kullanılamıyor. [Kütüphane, Vencord ve güncelleme ayrıntıları](docs/kutuphane-ve-guncellemeler.md), [Epic/Riot kapsamı](docs/epic-riot.md).

İlk sürüm Windows için geliştirilecek. Geliştirme ve performans referansı Windows 11 x64; Windows 10 desteği ayrıca doğrulanacak. Diğer işletim sistemleri sonraki aşamalara bırakıldı.

Discord yayın kapsamı: başlatma/durdurma, paylaşılan oyun veya pencereyi değiştirme, çözünürlük/kalite, FPS ve ses ayarları. Bunların tamamı ürün hedefi; her işlemin teknik erişimi ve kararlılığı ilk denemede ayrı doğrulanacak.

## Teknoloji kararı

| Katman | Seçim |
| --- | --- |
| Çekirdek ve işletim sistemi işlemleri | Rust |
| Masaüstü kabuğu ve arayüz | Slint 1.18.1 / Rust, native Windows pencere ve tepsi |
| Çizim | Software renderer; derlenen `.slint` bileşenleri |
| Yerel veri | SQLite / rusqlite |
| Discord | Yerel Vencord kullanıcı eklentisi, protokol 1 ve yetenek kontrolü |
| Gelecekteki eklentiler | Sürümlü protokol üzerinden ayrı süreçler |

Pencere açıkken 180 MiB, tepsi açıkken 40 MiB özel çalışma kümesi ve boşta %0,25 altında CPU başlangıç hedefidir. Native tam sürüm 1.000 kayıtta kapaksız 12,15 MiB / %0,141; kapaklı 30,80 MiB / %0,110 ölçüldü. Kapaklı doğrudan tepsi açılışı 23,29 MiB / %0,013. 50 pencere döngüsü ve 15 Rust testi tamamlandı. Ayrıntılar ve ölçüm sınırları [ilk sürüm belgesinde](docs/ilk-surum.md). Önceki Tauri/React uygulaması `apps/desktop` altında karşılaştırma amacıyla korunur; ana uygulama WebView içermez.

Epic/Riot eklendikten sonra 2.004 kayıtla pencere 32,32 MiB / %0,199; doğrudan tepsi 24,06 MiB / %0,028 ölçüldü. Bu önceki sürümün ölçümüdür. Yeni kütüphane/Vencord çalışmasında 35 Rust testi geçti. Ölçümler tek süreçli release uygulamasını kapsar; başlatılan oyun/launcher ve Discord kaynakları dahil değildir. [Yeni doğrulama kapsamı](docs/devam-plani.md).

## Çalıştırma

Windows x64 geliştirme uygulaması: `target/release/game-library-native.exe`. Pencereyi kapatmak veya Tepsiye al düğmesine basmak pencereyi gizler; uygulamadan çıkmak için Uygulama veya tepsi menüsündeki Çıkış kullanılır. Tepsi simgesine tıklamak kütüphaneyi yeniden açar. Paketleme ertelendi.

Native uygulama için Rust MSVC ve C++ Build Tools yeterlidir:

```powershell
cargo run -p game-library-native
```

Release derlemesi ve doğrulama:

```powershell
cargo build -p game-library-native --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Performans ekranında anlık örnek veya 60 saniyelik ölçüm alınabilir. Ölçüm yalnızca uygulamayı kapsar; Steam ve başlatılan oyunlar dahil edilmez. Raporlar `%LOCALAPPDATA%\GameLibrary\measurements\` altında saklanır. Kütüphane ekranında sürekli kaynak izleme yapılmaz.

Arayüz açık/tepsi ölçümlerini aynı release dosyasıyla almak için önce Game Library'den çık:

```powershell
pwsh -File scripts/measure.ps1 -Mode Window -Seconds 60
pwsh -File scripts/measure.ps1 -Mode Tray -Seconds 60
```

## Plan belgeleri

- [Mimari ve veri modeli](docs/mimari.md)
- [Dil, paketler ve araçlar](docs/paketler.md)
- [Oyun tarama ve kapak sistemi](docs/oyun-kesfi.md)
- [Epic ve Riot desteği](docs/epic-riot.md)
- [Discord ve istemci eklentileri](docs/discord.md)
- [Performans ve kararlılık kriterleri](docs/performans.md)
- [Arayüz yönü](docs/arayuz.md)
- [Geliştirme sırası](docs/yol-haritasi.md)
- [Yeni kütüphane, Vencord ve güncelleme akışları](docs/kutuphane-ve-guncellemeler.md)
- [Onaylanan devam kapsamı ve kabul işleri](docs/devam-plani.md)

Fixture testleri makinedeki kurulumlardan yalıtılır; gerçek launcher kabulü ayrıca yapılır. Yerel Steam kapaklarına ek olarak kullanıcı kapağı ve isteğe bağlı SteamGridDB araması desteklenir. API anahtarı Windows Credential Manager'da saklanır. Native arayüz 100 kayıtlık sayfalar ve sanallaştırılmış satırlar kullanır. JPEG/PNG kapaklar işçi thread'inde en fazla 180×270 piksele indirilir; dosya, boyut ve decode bellek sınırları uygulanır.

Uygulama güncelleyicisi Ed25519 imzalı release bildirimi ve SHA-256 doğrulaması kullanır. Bu geliştirme derlemesinde güven anahtarı ve yayımlanmış imzalı release yoktur; canlı otomatik güncelleme henüz etkin değildir. Vencord'un otomatik güncelleme tercihi yerel köprüden yönetilir.

Slint atfı Yardım → Hakkında ekranında `AboutSlint` bileşeniyle gösterilir. [Slint kullanım koşulları](https://slint.dev/terms-and-conditions).
