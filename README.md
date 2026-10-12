# Game Library

Bilgisayardaki oyunları tek yerde toplayan, yerel verilerle çalışan masaüstü kütüphanesi. Oyun bulma, başlatma, kapak yönetimi ve Discord bağlantıları ayrı modüller üzerinden geliştirilecek.

Öncelik sırası: kararlılık, düşük kaynak tüketimi, doğru oyun tespiti, kullanım kolaylığı ve görsel kalite.

## Proje durumu

12 Ekim 2026: mimari ve geliştirme planı hazır. Bu repo henüz çalışan bir uygulama, oyun tarayıcısı veya Discord entegrasyonu içermiyor. Paket kurulumu ve uygulama geliştirmesi ilk teknik doğrulama aşamasında başlayacak.

İlk sürüm için varsayım Windows 11 x64. Windows 10 desteği ayrıca doğrulanacak; diğer işletim sistemleri için çekirdek sınırları korunacak. Discord yayınında hangi kontrollerin gerektiği henüz açık.

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

Tauri seçimi ilk performans ölçümünden sonra kesinleşecek. Pencere açıkken 180 MiB, arayüz kapalı ve tepsi açıkken 40 MiB toplam özel çalışma kümesi başlangıç hedefidir; ölçülmüş sonuç değildir. Özellikle sürekli 50–80 MiB altında bir arayüz istenirse Rust/Slint alternatifi aynı yükle değerlendirilecek.

## Plan belgeleri

- [Mimari ve veri modeli](docs/mimari.md)
- [Dil, paketler ve araçlar](docs/paketler.md)
- [Oyun tarama ve kapak sistemi](docs/oyun-kesfi.md)
- [Discord ve istemci eklentileri](docs/discord.md)
- [Performans ve kararlılık kriterleri](docs/performans.md)
- [Arayüz yönü](docs/arayuz.md)
- [Geliştirme sırası](docs/yol-haritasi.md)

İlk geliştirilecek parça, gerçek Steam manifestiyle çalışan küçük bir masaüstü denemesi olacak. Kaynak bütçesi ve Discord erişimi doğrulandıktan sonra kapsam genişletilecek.
