# Game Library

Bilgisayardaki oyunları tek yerde toplayan, yerel verilerle çalışan masaüstü kütüphanesi. Oyun bulma, başlatma, kapak yönetimi ve Discord bağlantısı ayrı modüller üzerinden çalışır.

Öncelik sırası: kararlılık, düşük kaynak tüketimi, doğru oyun tespiti, kullanım kolaylığı ve görsel kalite.

## Proje durumu

14 Ekim 2026: eski executable'ın son sohbet kodunu içermediği doğrulandı. Discord komutları olayla iletiliyor; favori arkadaş paneli, profil görselleri ve yerel eklenti yöneticisi eklendi. Steam/Epic eşitleme yolları düzeltildi. Canlı Discord ve tam hesap listesi kabulü hâlâ bekliyor. [Düzeltmeler ve sınırlar](docs/duzeltmeler-20261014.md), [eklenti sözleşmesi](docs/eklenti-sistemi.md).

13 Ekim 2026 entegrasyon düzeltmesi: Discord'un kurulu köprüsü onarıldı ve canlı olarak 32 arkadaş/1 ses odası okundu. Steam/Epic ana bağlantı akışında anahtar/kod kopyalamadan resmi giriş penceresi ve otomatik hesap eşitlemesi var. OBS, RTX/NVIDIA Broadcast ve Spotify panelleri, isteğe bağlı Cloudflare DNS/GoodbyeDPI ve resmi paket güncellemesi eklendi. On marka/uygulama simgesi ChatGPT resim üretimiyle yenilendi. Gerçek hesap listesi, yayın/kayıt ve ağ değişikliği kabul sınırları [güncel doğrulama kaydında](docs/entegrasyonlar-20261013.md).

Arkadaş tıklamasının yanlış grup açma çağrısı düzeltildi; arkadaş listesi yanında native birebir sohbet paneli eklendi. Seçilen DM'nin son 30 metin mesajı gösterilir; Enter/Gönder işlemi aynı hesap/kişi/kanal kapsamında Discord'un kendi gönderim yoluna bağlıdır. Mesaj nonce'u geri okunmadan taslak temizlenmez. [Sohbet sözleşmesi ve doğrulama](docs/chat.md).

12 Ekim 2026: native Windows sürümü Steam/Epic/Riot keşfi ve başlatma, elle oyun ekleme, koleksiyonlar, kullanıcı başlık/not/kapak düzeltmeleri, süreçle doğrulanan oturumlar ve yerel Vencord bağlantısı içeriyor. Modern Steam kapak önbelleği ve görünür sayfanın eksik kapakları desteklenir; oyun ayrıntısında HowLongToBeat süreleri kaydedilir. Steam/Epic hesap eşitleme akışı hazır; kullanıcı hesap bağlantısını ertelediği için tam hesap listesi canlı olarak doğrulanmadı. Beholder: Conductor gerçek Steam başlatma testi geçti; Discord Stable bağlantısında arkadaş ve ses odası verisi okundu. Yayın kontrolleri bu istemcide kullanılamıyor. [Hesaplar, kapaklar ve süreler](docs/hesaplar-kapaklar-hltb.md), [kütüphane, Vencord ve güncellemeler](docs/kutuphane-ve-guncellemeler.md).

İlk sürüm Windows için geliştirilecek. Geliştirme ve performans referansı Windows 11 x64; Windows 10 desteği ayrıca doğrulanacak. Diğer işletim sistemleri sonraki aşamalara bırakıldı.

13 Ekim 2026: native arayüz sadeleştirildi. Oyun ekleme/tarama üstteki ⋯ menüsünde, kaynaklar ve performans Ayarlar altında bulunur. Discord arkadaşları son konuşmaya göre sıralanır; yıldızlanan arkadaşlar hesap bazında saklanır ve Favoriler sekmesinden süzülebilir. Konuşma sıralaması ve favoriler için güncel `gameLibraryBridge.desktop` eklentisi gerekir. [Bu çalışmanın kaydı](docs/kontrol-20261013.md).

Discord köprüsü değişen veriyi önbellekten günceller; boşta arkadaş/oda listelerini tekrar taramaz. Hesap bazında 50 favori ses odası, görünür odalar için native seçim/arama ve boş oda takibi eklendi. Native yayın panelinde başlatma, durdurma, kaynak değiştirme, kalite/FPS ve yayın sesi kontrolleri bulunur; tanınan istemci sözleşmesi ve kaynak desteği varsa etkinleşir. Kaynak listesi yalnızca seçim sırasında okunur; thumbnail veya video önizlemesi alınmaz. Sonuçlar Discord'dan geri okunur. Güncel köprü ve native uygulama birlikte kullanılmalıdır. Gerçek Discord oturumunda yeni kontrollerin kabulü bekliyor. [Yayın kontrolleri ve doğrulama sınırları](docs/yayin-kontrolleri-20261013.md).

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

Önceki kütüphane derlemesi 10.000 kapaklı kayıtta pencere açıkken 29,84 MiB / %0,011, doğrudan tepside 25,09 MiB / %0,004 ölçüldü. Discord bağlantısı bu ölçümde kapalıydı. Açılış p95 661,95 ms (OS paint hariç), çekirdek araması p95 15,93 ms (UI/IPC hariç). Sekiz saat raporu yaklaşık 59 dakikada kaldı; sonraki kontrolde süreç çalışmıyordu ve test tamamlanmış sayılmadı. Bu ölçümler yeni arayüzün performans sonucu değildir. [Ölçüm kaydı ve sınırlar](benchmarks/windows-native-user-library-20261012.json).

## Çalıştırma

Windows x64 geliştirme uygulaması: `target/release/game-library-native.exe`. Pencereyi kapatmak veya ⋯ menüsünden Tepsiye al seçmek pencereyi gizler; uygulamadan çıkmak için aynı menüdeki veya tepsi menüsündeki Çıkış kullanılır. Tepsi simgesine tıklamak kütüphaneyi yeniden açar. Paketleme ertelendi.

Native uygulama için Rust MSVC ve C++ Build Tools yeterlidir:

```powershell
cargo run -p game-library-native
```

Release derlemesi:

```powershell
cargo build -p game-library-native --release
```

Geliştirme sırasında doğrulama değişen işlevle sınırlanır; tam test paketi ve uzun performans ölçümleri her küçük değişiklikte tekrarlanmaz.

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
- [Hesap kütüphaneleri, kapaklar ve HowLongToBeat](docs/hesaplar-kapaklar-hltb.md)
- [Discord ve istemci eklentileri](docs/discord.md)
- [Performans ve kararlılık kriterleri](docs/performans.md)
- [Native arayüz ve görsel doğrulama](docs/arayuz.md)
- [Geliştirme sırası](docs/yol-haritasi.md)
- [Yeni kütüphane, Vencord ve güncelleme akışları](docs/kutuphane-ve-guncellemeler.md)
- [Onaylanan devam kapsamı ve kabul işleri](docs/devam-plani.md)

Fixture testleri makinedeki kurulumlardan yalıtılır; gerçek launcher kabulü ayrıca yapılır. Yerel Steam kapaklarına ek olarak kullanıcı kapağı ve isteğe bağlı SteamGridDB araması desteklenir. API anahtarı Windows Credential Manager'da saklanır. Native arayüz kapak odaklı kartlar, pencereye göre 4–6 sütun, 100 kayıtlık sayfalar ve sanallaştırılmış satırlar kullanır. Oyun ayrıntısı Genel bakış/Düzenle olarak ayrılır; Discord arkadaş ve ses odalarını liste halinde gösterir. JPEG/PNG kapaklar işçi thread'inde en fazla 180×270 piksele indirilir; dosya, boyut ve decode bellek sınırları uygulanır.

Uygulama güncelleyicisi Ed25519 imzalı release bildirimi ve SHA-256 doğrulaması kullanır. Bu geliştirme derlemesinde güven anahtarı ve yayımlanmış imzalı release yoktur; canlı otomatik güncelleme henüz etkin değildir. Özel GameLibraryBridge paketini korumak için standart Vencord otomatik güncellemesi kapalıdır; **Köprüyü hazırla / güncelle** resmi Vencord kaynağını günceller, köprüyü derler ve kurulu paketi yedekleyerek uygular.

Slint atfı ⋯ → Hakkında ekranında `AboutSlint` bileşeniyle gösterilir. [Slint kullanım koşulları](https://slint.dev/terms-and-conditions).
