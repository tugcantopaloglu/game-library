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
