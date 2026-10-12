# Dil, paketler ve araçlar

12 Ekim 2026 tarihinde npm ve crates.io kayıtları kontrol edildi. Aşağıdaki sürümler araştırma anındaki kararlı sürümlerdir; birlikte derlenmiş bir bağımlılık seti değildir. İlk denemede uyum doğrulanacak, gerçek sürümler `Cargo.lock` ve `pnpm-lock.yaml` ile sabitlenecek.

## Rust

Ana uygulama native Slint'e geçti. `slint` ve `slint-build` 1.18.1 sabit sürümle, varsayılan feature'lar kapalı olarak kullanılıyor. Seçilen özellikler winit, software renderer, erişilebilirlik, native tepsi ve Windows pencere handle erişimi. Native klasör seçimi `rfd` 0.16, thumbnail decode `image` 0.25 (JPEG/PNG) üzerinden. Normal kullanımda Node, React, WebView2 veya ağ runtime'ı başlamaz. Aşağıdaki Tauri ve web paketleri önceki karşılaştırma sürümüne aittir; diğer paketler sonraki aşamaların adaylarıdır.

| Paket | Kontrol edilen sürüm | Kullanım |
| --- | --- | --- |
| tauri | 2.12.3 | Pencere, IPC, tepsi ve paketleme |
| tokio | 1.53.2 | Sınırlı eşzamanlı işler ve ağ; yalnızca gereken feature'lar |
| rusqlite | 0.40.2 | SQLite; bundled SQLite ve gerekli sorgu özellikleri |
| notify | 8.2.0 | Manifest klasörlerinde işletim sistemi dosya olayları |
| reqwest | 0.13.5 | Metadata ve görsel indirme; Rustls, timeout ve sınırlı redirect |
| serde | 1.0.229 | Tipli veri sözleşmeleri |
| thiserror | 2.0.21 | Çekirdek hata tipleri |
| tracing | 0.1.44 | Yerel, boyutu sınırlı tanılama |
| keyring | 4.2.0 | Windows Credential Manager erişimi |
| windows | 0.62.2 | Registry, süreç ve Windows API'leri; seçili feature'lar |
| image | 0.25.10 | Gerektiğinde küçük kapak varyantları; yalnızca JPEG/PNG/WebP |
| keyvalues-parser | 0.2.4 | Steam VDF/ACF ayrıştırması için aday |

`serde_json`, `tokio-util`, `tracing-subscriber`, `tracing-appender` ve `ts-rs` gerektiği aşamada eklenecek. VDF ayrıştırıcısı büyük dosya, bozuk girdi ve tekrar eden alan fixture'larıyla doğrulanmadan kalıcı seçim sayılmayacak.

`notify` bütün diski izlemeyecek; yalnızca küçük manifest köklerine abone olacak. [notify belgeleri](https://docs.rs/notify/latest/notify/). SQLite sorguları ve işlemleri doğrudan yönetilecek. [rusqlite belgeleri](https://docs.rs/rusqlite/latest/rusqlite/).

Tauri'nin single-instance, opener ve updater Rust eklentileri ilgili aşamada değerlendirilecek. Single-instance için bir npm paketi gerekmiyor. Genel shell, SQL ve dosya sistemi yetkisini UI'ya açan paketler başlangıç listesine alınmayacak.

## Arayüz

| Paket | Kontrol edilen sürüm | Kullanım |
| --- | --- | --- |
| react / react-dom | 19.3.0 | Görünüm katmanı |
| typescript | 7.0.2 | Tip denetimi; runtime bağımlılığı değil |
| vite | 8.3.4 | Geliştirme ve statik üretim derlemesi |
| @vitejs/plugin-react | 6.1.2 | React derleme entegrasyonu |
| @tauri-apps/api | 2.12.3 | Tipli masaüstü komutları |
| @tauri-apps/cli | 2.12.2 | Geliştirme ve paketleme aracı |
| tailwindcss / @tailwindcss/vite | 4.3.3 | Üretimde statik CSS |
| @tanstack/react-virtual | 3.14.15 | Büyük kütüphanede görünür satırların render edilmesi |
| radix-ui | 1.8.0 | Dialog, menü ve tooltip gibi seçili erişilebilir etkileşimler |
| lucide-react | 1.55.0 | Yalnızca kullanılan ikonların import edilmesi |
| zustand | 5.0.15 | Küçük görünüm durumu; ilk ihtiyaç ortaya çıkarsa |

Arayüz runtime'ında Node.js sunucusu bulunmayacak. Vite ve pnpm yalnızca geliştirme/derleme araçlarıdır. İlk sürümde çok sayıda ekran olmadığı için router ve ayrı bir veri sorgulama kütüphanesi zorunlu değil.

Radix ve 21st.dev bileşenleri topluca alınmayacak. Gerekli parçalar seçilecek, üretilen bundle incelenecek. Animasyon kütüphanesi, grafik paketi, WebGL arka planı veya tam UI teması başlangıç bağımlılığı olmayacak.

## Discord ve eklentiler

Social SDK için Discord'un sağladığı C++ SDK kullanılacak; doğrulanmamış Rust wrapper temel bağımlılık yapılmayacak. SDK sürümü ve lisans/dağıtım biçimi Developer Portal erişimiyle kesinleşecek.

Sunucu botu seçeneği gerekiyorsa Rust `twilight` ekosistemi değerlendirilecek. Her kullanıcıya aynı bot tokenını dağıtan mimari kurulmayacak. Kişisel denemede kullanıcının kendi botu, dağıtımda ayrı bir servis seçeneği var.

Vencord kurulum yöneticisi kendi patch yöntemini yazmak yerine resmi installer'ı kullanacak. WASM runtime, üçüncü taraf eklentiler gerçekten gerektiğinde değerlendirilecek; ilk sürümde boşta runtime tutulmayacak.

## Geliştirme araçları

- Rust stable/MSVC toolchain, rustfmt ve clippy.
- Node.js LTS ve pnpm; yerel geliştirmenin toolchain sürümü sabitlenecek.
- Windows için C++ Build Tools ve WebView2; Social SDK yardımcı süreci gerekirse CMake.
- Rust parser ve veri geçişleri için fixture tabanlı testler.
- Kritik arayüz davranışları için Vitest/Testing Library; gerçek masaüstü yaşam döngüsü ayrıca Windows üzerinde doğrulanacak.
- Paket güncelleme, lisans ve bağımlılık denetimi; kullanıcı makinelerinde otomatik paket çözümleme yapılmayacak.

TypeScript 7, Vite 8 ve React 19 ilk Windows sürümünde birlikte derlendi. Kesin bağımlılıklar lockfile'larda kayıtlı. Bu deneme yalnızca React, Tauri API, sanallaştırma ve Lucide kullanıyor; stiller statik CSS, oyun ayrıntısı yerel HTML dialog ile uygulanıyor. Tailwind/Radix ve diğer adaylar ihtiyaç doğduğunda eklenecek. Yeni sürüm numarası performans veya kararlılık gerekçesi sayılmayacak.
