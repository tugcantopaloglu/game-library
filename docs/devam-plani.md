# Geliştirme kapsamı — 12 Ekim 2026

Kullanıcı 1, 2, 4 ve 5 numaralı işleri onayladı. Diğer launcher adaptörleri bu çalışma kapsamına alınmadı. Discord Developer Portal uygulaması yok; yerel Discord/Vencord bağlantısı kullanılacak. Resmi Vencord kaynağı https://vencord.dev/ ve https://github.com/Vencord/Installer.

## Kabul işleri

- [x] Steam fixture taraması makinedeki kurulumlardan yalıtıldı. 35 çalışma alanı testi, Clippy ve release derlemesi geçti.
- [x] Elle executable ekleme, kullanıcı başlık/not/kapak düzeltmeleri, koleksiyonlar ve oyun ayrıntısı uygulandı. Tarama/yeniden açılışta koruma test edildi.
- [x] Başlatma isteği ile gerçek süreç ayrıldı. Beholder: Conductor Steam üzerinden açıldı; seçilen executable 23,05 saniyede gözlendi.
- [x] Yerel kapak, sınırlandırılmış ağ/önbellek, Credential Manager anahtarı ve native odak kontrolleri uygulandı. SteamGridDB canlı API kabulü anahtar olmadığı için bekliyor; klavyeyle uçtan uca gezinme ayrıca bekliyor.
- [x] Yerel Vencord köprüsü gerçek oturuma bağlandı; 32 arkadaş ve 1 ses odası okundu. Ses/kanal/DM/güncelleme fonksiyonları bulundu. Durum değiştiren ses işlemleri test edilmedi; yayın yeteneklerinin tamamı kapalı.
- [x] Resmi installer ve özel kaynak derlemesi yönetimi uygulandı. Discord Stable kurulumu ve canlı bağlantı doğrulandı. Electron'a verilen canonical yol öneki kaynaklı açılış hatası düzeltildi. PTB/Canary ve bağımsız onar/kaldır kabulü bekliyor.
- [x] Vencord otomatik güncelleme tercihi okundu; uygulama için imzalı bildirim, SHA-256, staging ve açılış doğrulamalı executable geri dönüşü uygulandı. İlk yayımlanmış imzalı sürüm zinciri ve tam veri geri dönüşü kabulü bekliyor.
- [x] 10.000 kapaklı kayıtla iki kesintisiz 60 saniyelik ölçüm tamamlandı: pencere 29,84 MiB / %0,011 CPU; doğrudan tepsi 25,09 MiB / %0,004 CPU. Discord bağlantısı kapalı, yalnızca native süreç ölçüldü. 100 Rust aramasında p95 15,93 ms; 20 süreç açılışında thumbnail/software render dahil p95 661,95 ms. UI/IPC arama gecikmesi ve OS paint süresi dahil değil. Slint 1,5/2,0 ölçekli görüntüler incelendi; canlı Windows DPI geçişi testi bekliyor.
- [ ] Sekiz saat kararlılık sonucu, gerçek Epic/Riot açılışları ve oyun açıkken frame-time/GPU A/B ölçümü.

Son ölçümler ve sınırlar [benchmark kaydında](../benchmarks/windows-native-user-library-20261012.json). Kararlılık testi 12 Ekim 21:28:45'te ayrı fixture profiliyle tepside başladı; beklenen bitiş 13 Ekim yaklaşık 05:29 (Türkiye saati). Sonuç `artifacts/user-library-stability-8h.json` içine dakikalık ve sonunda `completed: true` olarak yazılır. Uyku/kesinti işaretlenirse kabul edilmez. Başlatılmış olması tamamlandığı anlamına gelmez.

## Teknik sınırlar

Vencord köprüsü masaüstü kullanıcı eklentisidir. Standart installer özel eklentiyi içermez; geliştirme derlemesine kurulumu ayrıca yönetilir. Discord tokenı okunmaz veya uygulamaya taşınmaz. İstemci API'si değiştiğinde desteklenmeyen yetenek kapanır. Güncelleme sırasında köprü sürüm uyumu tekrar denetlenir.

Genel dağıtım, imzalama anahtarı ve ilk yayımlanmış sürüm olmadan uygulama güncelleme zinciri canlı yayınla doğrulanamaz. Yerel doğrulama ve güncelleme altyapısı bu çalışma içinde hazırlanır.

Snyk talimatı geçerlidir. Bu oturumda `snyk_code_scan` aracı ve Snyk CLI yok; plugin araması da sonuç vermedi. Bu sınırlama derleme, Clippy, bağımlılık ve kritik güvenlik testlerinin yerine geçtiği şeklinde raporlanmayacak.

## Kapaklar ve native arayüz devamı

- [x] Altı kaynak uyarısının Epic/Riot metadata kaynaklı olduğu belirlendi; gerçek tarama artık 0 uyarı, 8 Steam, 1 erişilemeyen Epic ve 3 Riot kaydı veriyor. Steam'in iki kütüphane kökü ve sekiz yerel kapağı doğrulandı.
- [x] Steam/Epic tam hesap kütüphanesi için ayrı eşitleme, Credential Manager anahtarı ve Legendary yardımcı akışı hazırlandı. Kullanıcı hesap bağlantısını erteledi; canlı tam hesap kabulü bekliyor.
- [x] HowLongToBeat açık sayfalarından üç süre kategorisi kaydedildi; Baldur's Gate 3, Hogwarts Legacy ve VALORANT eşleşmeleri doğrulandı.
- [x] 21st.dev örnekleri incelenerek native Slint kütüphane, oyun ayrıntısı, kaynaklar, koleksiyonlar, Discord ve ayarlar yeniden düzenlendi. Tasarım sistemi ve geliştirme görüntüleme seçenekleri [arayüz belgesinde](arayuz.md).
- [ ] Önceki sekiz saat raporu yaklaşık 59 dakikada kaldı ve sonraki kontrolde süreç bulunamadı. Tamamlanma veya yeni derleme kararlılığı kabulü yapılmadı.

Ayrıntılar: [hesaplar, kapaklar ve HowLongToBeat](hesaplar-kapaklar-hltb.md). Önceki 10.000 oyun RAM/CPU ölçümleri yeni arayüzün performans sonucu olarak sunulmaz.
