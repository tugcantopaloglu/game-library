# Hesap kütüphaneleri, kapaklar ve HowLongToBeat

12 Ekim 2026 devam kapsamı: kaynak uyarılarının doğru yerde gösterilmesi, Steam/Epic kayıtlarının denetlenmesi, kapakların bulunması ve bitirme süreleri. Kullanıcı kurulu olmayan hesap oyunlarının da görünmesini istedi.

## Kaynak taraması

Altı uyarı Discord'a ait değildi: iki Epic kaydında executable boştu; dört Riot metadata dizini oyun ayar dosyası içermiyordu. Tarama sonucu artık kütüphane/kaynak ekranlarında ayrı gösterilir. Kaynak uyarıları `[Steam]`, `[Epic Games]`, `[Riot Games]` ile ayrılır; her kaynak ekranı kendi uyarılarını gösterir.

Epic URI başlatması executable'ı launcher üzerinden çözer. Boş executable alanı geçerli oyun kaydını dışlamaz; alan doluysa kurulum dışına çıkma denetimi korunur. Eksik kurulum veya yarım indirme kurulu sayılmaz. Unreal Engine, eklenti ve yazılım kategorileri oyun olarak eklenmez. Riot'un boş placeholder ve patch metadata dizinleri normal oyun kaydı gibi işlenmez; gerçek okuma hataları uyarı üretmeye devam eder.

Steam'in `libraryfolders.vdf` kökleri taranır. Steamworks Common Redistributables oyun listesine alınmaz. Var olan böyle bir kayıt kullanıcı düzeltmesi/favori/koleksiyon içermiyorsa temizlenir. Hesap kaydı daha sonra kurulduğunda aynı oyun kimliği kullanılır; tekrar taramada çoğalmaz, favori/not/koleksiyonlar korunur.

## Hesaptaki oyunlar

Yerel manifestler kurulu veya daha önce kaydedilmiş kurulumları gösterir. Hesabın tam listesi için ayrı eşitleme gerekir; launcher'daki genel metadata önbelleği sahiplik kanıtı sayılmaz.

Steam ekranında **Steam'e bağlan** resmi Steam giriş sayfasını ayrı Chrome/Edge profilinde açar. Oturum ve Steam Guard kullanıcı tarafından tamamlanır; hesap listesi ardından otomatik eşitlenir. SteamID/API anahtarı kopyalamak gerekmez. Parola/token tarayıcıda kalır; native uygulamaya yalnız doğrulanan hesap kimliği ve oyun kimliği/adı gelir. **Kütüphaneyi eşitle** aynı ayrı oturumla listeyi yeniler; oturum süresi dolmuşsa yeniden giriş gerekir. Anahtar yöntemi gelişmiş seçeneklerde korunur ve anahtar Windows Credential Manager'da saklanır. Yerel `loginusers.vdf` kimliği varsa MostRecent, tek AutoLogin veya tek hesap üzerinden bulunur; birden fazla olası hesapta tahmin edilmez. Erişilemeyen veya eksik yanıt eski listeyi sıfırlamaz. [Steam GetOwnedGames](https://partner.steamgames.com/doc/webapi/IPlayerService), [anahtar kullanımı](https://partner.steamgames.com/doc/webapi_overview/auth).

Epic ekranında **Hesabı bağla** resmi Epic WebView giriş penceresini açar. Oturum tamamlandığında hesap listesi doğrulanıp otomatik eşitlenir; authorizationCode kopyalaması ve terminal adımı kaldırıldı. Giriş için [Legendary](https://github.com/legendary-gl/legendary) ayrı yardımcıdır. Windows x64 dosyası resmi proje GitHub release'inden gelir; yayımlanan SHA-256 ve PE başlığı doğrulanır. Oturum yardımcının ayrı config dizininde kalır. Epic launcher tokenı içe aktarılmaz. JSON oyun listesinde ana oyunlar ve üçüncü taraf launcher oyunları alınır; DLC/Unreal içerikleri ayrı oyun olarak eklenmez. İlk canlı hesap kabulü kullanıcı girişinden sonra yapılır. [WebView doğrulaması ve sınırlamalar](epic-login.md).

Kurulu olmayan hesap oyunları **Kurulu değil** etiketiyle gösterilir. Oyna yalnızca kurulum doğrulanan oyunlarda kullanılabilir. Hesap eşitlemesi oyun kurmaz, güncellemez veya bulut kayıtlarını değiştirmez. Bağlı hesaplar kütüphane taramasından sonra ve kullanıcı eşitleme düğmesiyle yenilenir.

## Kapaklar

Steam'in eski `<AppID>_library_600x900` dosyaları, yeni `<AppID>/library_600x900` ve `<AppID>/<hash>/library_capsule.jpg` biçimleri desteklenir. Tek oyun klasöründe en fazla 128 hash dizini incelenir; tüm disk taranmaz.

Eksik kapaklar yalnızca görünür sayfa için, tek arka plan işçisi ve 100 kayıtlık kuyrukla hazırlanır. Steam'de AppID üzerinden CDN, Epic'te hesap metadata'sındaki görseller, eşleşme varsa HowLongToBeat'in oyun kapağı kullanılır. HTTPS/host, yönlendirme, 8 MiB dosya ve decode sınırları korunur. Kullanıcının seçtiği kapak önceliklidir; otomatik kapaklar temel kaynak görselini değiştirir. Bir oturumda aynı oyun için sınırsız tekrar yapılmaz. Başka bir ağ işiyle birlikte en fazla iki işçi indirme yapar. Gelen kapak yalnızca ilgili kartı günceller; tüm sayfa tekrar decode edilmez.

## HowLongToBeat

Oyun ayrıntısı ana hikâye, ana + ek içerik ve tamamlama sürelerini gösterir. Veriler süre kategorileri olarak saniye cinsinden kaydedilir; saat gösterimine çevrilir. Sıfır/eksik değer **Veri yok** olur; tahmini süre üretilmez.

Steam AppID için [Wikidata'nın Steam/HowLongToBeat kimlik eşleştirmesi](https://www.wikidata.org/wiki/Property:P2816) kullanılır; otomatik eşleşmenin HowLongToBeat sayfasındaki Steam kimliği de doğrulanır. Diğer oyunlarda tek bir tam ad eşleşmesi aranır. Birden fazla/eksik eşleşme kullanıcıya bırakılır. **Oyunu sitede ara** ve oyun URL'si/ID alanı yanlış veya bulunamayan eşleşmeyi düzeltir. Kaynak sayfası ve eşleşen oyun adı görünür.

Süreler açık oyun sayfasındaki yapılandırılmış veriden alınır; korumalı arama API'si kullanılmaz. Kaynak biçimi veya erişim değişirse hata gösterilir. Kayıtlı süreler sonraki açılış ve taramalarda kalır; yenileme kullanıcının düğmesiyle yapılır. Site erişimi olmadan kaydedilmiş veriler görüntülenebilir. [Örnek kaynak sayfası](https://howlongtobeat.com/game/68033).

## Kabul sınırları

- Fixture testleri hesap/kurulum birleşmesini, kalıcı kullanıcı verisini, eksik hesap yanıtını, modern kapak dizinini ve gerçek hata örneklerini kapsar.
- Çalışma alanında 42 test ve uyarısız Clippy geçti. Kullanıcı hesap bağlantılarını erteledi; Steam/Epic tam hesap listesinin canlı kabulü bekliyor.
- Gerçek tarama ilk doğrulamada sıfır uyarı verdi: 8 Steam oyunu, 1 erişilemeyen Epic kurulumu ve 3 Riot kaydı. Tam hesap sayısı giriş/eşitlemeden sonra doğrulanır.
- Baldur's Gate 3 süreleri gerçek kaynak sayfasından alındı. Tüm kütüphanenin otomatik eşleşeceği varsayılmaz; eksik metadata elle düzeltilebilir.
- Hogwarts Legacy ve VALORANT için de gerçek açık sayfa eşleşmesi doğrulandı. Tam ad eşleştirmesi Wikidata'nın İngilizce ve evrensel (`@mul`) etiketlerini kapsar.
- Önceki derlemenin sekiz saat raporu yaklaşık 59 dakikada kaldı; süreç sonraki kontrolde çalışmıyordu. Test tamamlanmadı ve bu yeni değişikliklerin uzun çalışma kabulü yerine geçmez.
- `snyk_code_scan` ve Snyk CLI bu oturumda mevcut değil; güvenlik taraması tamamlanmış sayılmıyor.
