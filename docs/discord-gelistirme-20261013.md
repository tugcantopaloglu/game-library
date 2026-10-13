# Discord geliştirmeleri — 13 Ekim 2026

## Köprü optimizasyonu

Arkadaşlar, odalar ve ses/yayın kontrolleri ayrı önbelleklerde tutulur. Flux değişiklikleri aynı komut aralığında birleştirilir; arkadaş aktivitesi oda listesini tekrar oluşturmaz. Modül araması önbelleğe alınır. Eklenti kapanırken store ve ayar dinleyicileri kaldırılır. Dinleyici desteği eksik istemcide dört saniyede bir sınırlı yeniden okuma yapılır.

Değişmeyen durumda renderer mevcut snapshot sürümünü küçük bir `snapshot_unchanged` mesajıyla onaylar. Native köprü yalnızca eşleşmiş bağlantıda, önceden alınmış aynı sürümün onayını kabul eder. Transport heartbeat'i eski veriyi canlı tutmaz; renderer durmuşsa altı saniyelik geçerlilik sınırı korunur. Yeniden bağlantıda tam durum gönderilir. Eski köprülerin tam snapshot akışı desteklenmeye devam eder; yeni eklenti güncel native uygulamayı gerektirir.

Native işçi veri sürümü ve bağlantı durumu değişmemişse JSON kopyası, SQLite favori okuması ve Slint modellerinin yeniden kurulmasını atlar. Göreli konuşma zamanları dakikada bir yenilenir; yeni timer eklenmez. Kullanıcının favori/filtre işlemi görünümü hemen yeniler.

Kısa kontrol: iki Node önbellek/dinleyici testi ve dört Rust köprü testi geçti. 100 değişmeyen çağrıda her veri bölümü yalnızca bir kez okundu; yeniden bağlantı tam yenileme yaptı. Eklenti dosyaları resmi Vencord kaynağıyla yalnızca köprüye yönelik TypeScript kontrolünden geçti. Tam test paketi ve uzun kaynak ölçümü yapılmadı. Gerçek Discord kurulumu değiştirilmedi.

Store sözleşmesi [Vencord FluxStore tiplerinde](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/FluxStore.d.ts), ayar dinleyicileri [SettingsStore kaynağında](https://github.com/Vendicated/Vencord/blob/main/src/shared/SettingsStore.ts) tanımlıdır.

## Favori odalar

En fazla 50 ses odası Discord hesabına göre SQLite'ta takip edilir. Dolu odalar ve favoriler ayrı süzülür. Favori oda boşken listede kalır; silinen veya erişimi kaybolan oda kullanılamıyor olarak gösterilir ve takipten çıkarılabilir. Kişi sayısı bilinmiyorsa sıfır olarak gösterilmez.

Oda seçici yalnızca açıldığında yerel Discord kanallarından görünür ses/stage odalarını arar; 100 sonuç ve 128 karakterlik arama sınırı vardır. Seçici kapanınca, Discord sayfasından çıkınca veya pencere tepsiye alınca katalog temizlenir. Görünürlük ve katılma izinleri ayrı denetlenir; izin değişikliği oda önbelleğini geçersiz kılar. Bildirim veya otomatik odaya katılma eklenmedi.

Kısa kontrol: hesap ayrımı/yeniden açılış/50 oda sınırı için bir SQLite testi ve boş/erişilemeyen oda görünümü için bir native test geçti. Güncel köprü gerçek Vencord tipleriyle kontrol edildi. Canlı Discord oda işlemleri bu aşamada tetiklenmedi.

## Yayın kontrolleri

Discord sayfasındaki Yayın eylemi native durum panelini açar. Panel mevcut kaynak adını, çözünürlüğü, FPS'i, yayın sesi durumunu ve bekleyen işlemi gösterir. Kalite/FPS seçenekleri ve durdurma yalnızca köprünün ilgili yeteneği varsa etkinleşir. Video yakalama veya kodlama Game Library sürecine taşınmadı.

Köprü aktif yayının anahtarını RTC kayıtlarından kendi hesap/kanal eşleşmesiyle bulur. Önceki kodun aktif yayın nesnesinde doğrudan `streamKey` beklemesi kaldırıldı. Medya kaynağı, context ve setter sözleşmesi okunabiliyorsa kalite/FPS isteği mevcut kaynağın diğer alanlarını korur. Başlangıç seçenekleri düşük kalite düzeyleri ve aynı yayında önceden gözlenen düzeylerle sınırlıdır. Tanınmayan kaynak veya değişmiş yayın anahtarıyla işlem yapılmaz.

İşlem yalnızca kuyruktan gönderildiğinde başarı gösterilmez. Beş saniye içinde gerçek kaynak kalite/FPS değeri veya yayının kapanışı geri okunur. Sonuç doğrulanmazsa hata durumu gösterilir; kalite/FPS değişimi aynı yayında önceki kaynağa geri alınmaya çalışılır. Hesap/yayın değişmişse eski işlem yeni yayına uygulanmaz. İstemci çağrısı üç saniyede zaman aşımına uğrar. Değişmeyen kontrol olayları aynı veriyi tekrar göndermez.

Bu ortamda kullanılabilir canlı Discord/Vencord oturumu bulunmadı. Yayın başlatma, yeni kaynak seçimi ve yayın sesini değiştirme için güncel istemci çağrı sözleşmeleri burada doğrulanamadı; bu üç yetenek kapalıdır. Yayın sesi yalnızca okunur. Bu aşama bu işlemleri tamamlanmış veya canlı olarak çalışmış saymaz.

Kısa kontrol: dört Node yayın kontrol testi, beş önbellek/IPC testi ve bir Rust yayın isteği doğrulama testi geçti. İstek sınırları, desteklenmeyen kalite, eski yayın anahtarı, geri okuma/zaman aşımı ve hesap değişimi kontrol edildi. Kısmi veri okuma hatası önbelleği yarım güncellemez; gönderilemeyen sürüm tam veri olarak yeniden gönderilir. UTF-8 mesaj bütçesi favoriler korunarak uygulanır ve bozuk surrogate değerleri düzeltilir. Süresi geçmiş durum yeniden onaylandığında native görünümün yenilenmesi için bir Rust regresyon kontrolü yapıldı. Köprü gerçek Vencord tipleriyle kontrol edildi. Tam test paketi, uzun ölçüm, gerçek yayın/ses değişikliği veya Discord kurulumu yapılmadı.

Native görünüm örnek IPC verisiyle açıldı; 720p/30 FPS yayın paneli ile dolu, boş ve erişilemeyen üç favori oda görüntülendi. Görüntü `artifacts/discord-development-20261013/native-discord.jpg` içindedir ve “Örnek veri” etiketlidir. İlk derlemede açık eski executable dosya kopyasını engelledi; son kaynaklarla release derlemesi başarıyla tamamlandı. `target/release/game-library-native.exe` ve `target/release/game-library-native-next.exe` aynı güncel binary'yi içerir. SHA-256: `CD437FE659B8D1B6DC2A27FD4E7F04D39DF1F644E38F4E535F68508E2FC41D90`. Açık önceki sürüm varsa yeni dosyayı açmadan önce uygulamadan Çıkış seçilmelidir.

Geliştirmeler ayrı commitlerde kaydedildi: köprü `fe7da99`, favori odalar `043d41f`, mevcut yayın kontrolleri `3fce705`. Native uygulama ve köprü birlikte güncellenmelidir.

Kaynak/kontrol veri sözleşmeleri [MediaEngineStore](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/MediaEngineStore.d.ts) ve [ApplicationStreamingStore](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/ApplicationStreamingStore.d.ts) tiplerinden incelendi. Discord'un özel action modüllerinin canlı uyumu ayrıca kontrol edilmelidir.
