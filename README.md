# Game Library

Bilgisayardaki oyunları tek yerde toplayan, yerel verilerle çalışan masaüstü kütüphanesi. Oyun bulma, başlatma, kapak yönetimi ve Discord bağlantıları ayrı modüller üzerinden geliştirilecek.

Öncelik sırası: kararlılık, düşük kaynak tüketimi, doğru oyun tespiti, kullanım kolaylığı ve görsel kalite.

## Proje durumu

12 Ekim 2026: ilk Windows sürümü geliştirildi. Steam kütüphane keşfi, kurulum doğrulaması, sayfalı oyun listesi, arama, favoriler, Steam üzerinden başlatma ve RAM/CPU ölçümü içeriyor. Discord ve diğer launcher adaptörleri sonraki aşamalarda.

İlk sürüm Windows için geliştirilecek. Geliştirme ve performans referansı Windows 11 x64; Windows 10 desteği ayrıca doğrulanacak. Diğer işletim sistemleri sonraki aşamalara bırakıldı.

Discord yayın kapsamı: başlatma/durdurma, paylaşılan oyun veya pencereyi değiştirme, çözünürlük/kalite, FPS ve ses ayarları. Bunların tamamı ürün hedefi; her işlemin teknik erişimi ve kararlılığı ilk denemede ayrı doğrulanacak.

## Teknoloji kararı

| Katman | Seçim |
| --- | --- |
| Çekirdek ve işletim sistemi işlemleri | Rust |
| Masaüstü kabuğu ve arayüz | Slint 1.18.1 / Rust, native Windows pencere ve tepsi |
| Çizim | Software renderer; derlenen `.slint` bileşenleri |
| Yerel veri | SQLite / rusqlite |
| Discord | Yeteneğe göre Social SDK, onaylı RPC veya sunucu botu |
| Gelecekteki eklentiler | Sürümlü protokol üzerinden ayrı süreçler |

Pencere açıkken 180 MiB, tepsi açıkken 40 MiB özel çalışma kümesi ve boşta %0,25 altında CPU başlangıç hedefidir. Native karşılaştırma denemesi 1.000 kapaksız kayıtta 10,84 MiB özel RAM ve %0,123 CPU ölçüldü. Ana sürümün sonuçları [ilk sürüm belgesinde](docs/ilk-surum.md) takip edilir. Önceki Tauri/React uygulaması `apps/desktop` altında karşılaştırma amacıyla korunur; ana uygulama WebView içermez.

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
- [Discord ve istemci eklentileri](docs/discord.md)
- [Performans ve kararlılık kriterleri](docs/performans.md)
- [Arayüz yönü](docs/arayuz.md)
- [Geliştirme sırası](docs/yol-haritasi.md)

Steam kurulu olmayan ortamda fixture testleri uygulanır. Gerçek oyun başlatma kabul testi Steam'in kurulu olduğu bilgisayarda yapılmalıdır. İlk sürüm internetten kapak indirmez; Steam'in mevcut yerel kapak dosyalarını bulursa gösterir. Native arayüz 100 kayıtlık sayfalar ve sanallaştırılmış satırlar kullanır. JPEG/PNG kapaklar işçi thread'inde en fazla 180×270 piksele indirilir; dosya, boyut ve decode bellek sınırları uygulanır.

Slint atfı Yardım → Hakkında ekranında `AboutSlint` bileşeniyle gösterilir. [Slint kullanım koşulları](https://slint.dev/terms-and-conditions).
