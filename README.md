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
| Masaüstü kabuğu | Tauri 2 |
| Arayüz | React, TypeScript, Vite |
| Stil ve etkileşim | Tailwind CSS, seçili Radix bileşenleri, Lucide |
| Yerel veri | SQLite / rusqlite |
| Discord | Yeteneğe göre Social SDK, onaylı RPC veya sunucu botu |
| Gelecekteki eklentiler | Sürümlü protokol üzerinden ayrı süreçler |

Pencere açıkken 180 MiB, arayüz kapalı ve tepsi açıkken 40 MiB toplam özel çalışma kümesi başlangıç hedefidir. İlk ölçüm protokolü ve sonuçlar [ilk sürüm belgesinde](docs/ilk-surum.md) takip edilir. Özellikle sürekli 50–80 MiB altında bir arayüz istenirse Rust/Slint alternatifi aynı yükle değerlendirilecek.

## Çalıştırma

Windows x64 release uygulaması: `target/release/game-library.exe`. WebView2 Runtime gerekir. Pencereyi kapatmak veya Tepsiye al düğmesine basmak arayüzü kapatır; uygulamadan çıkmak için tepsi menüsündeki Çıkış kullanılır. Tepsi simgesine tıklamak kütüphaneyi yeniden açar.

Geliştirme için Rust MSVC, C++ Build Tools, WebView2, Node.js ve pnpm:

```powershell
pnpm install --frozen-lockfile
pnpm dev
```

Release derlemesi ve doğrulama:

```powershell
pnpm build
pnpm check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Performans ekranında 60 saniyelik ölçüm başlatılabilir. Ölçüm yalnızca uygulama ve WebView2 süreçlerini kapsar; Steam ve başlatılan oyunlar dahil edilmez. Raporlar `%LOCALAPPDATA%\GameLibrary\measurements\` altında saklanır. Kütüphane ekranında sürekli kaynak izleme yapılmaz.

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

Steam kurulu olmayan ortamda fixture testleri uygulanır. Gerçek oyun başlatma kabul testi Steam'in kurulu olduğu bilgisayarda yapılmalıdır. İlk sürüm internetten kapak indirmez; Steam'in mevcut yerel kapak dosyalarını bulursa gösterir.
