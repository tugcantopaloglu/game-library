# 13 Ekim 2026 entegrasyon düzeltmeleri

## Hesap kütüphaneleri

Steam'in ana bağlantı düğmesi resmi giriş sayfasını ayrı Edge/Chrome profilinde açar. Kullanıcı Steam Guard dahil girişini tamamlar; kurulu olmayan oyunlar da otomatik eşitlenir. Native uygulama yalnız SteamID ve doğrulanan oyun adı/AppID listesini alır. Giriş bilgileri tarayıcı profilinde kalır; normal tarayıcı profiline veya Steam launcher tokenına erişilmez. Gelişmiş Web API anahtarı yöntemi korunur. Bağlı hesaplar tarama sonrasında da eşitlenir; hata/eksik yanıt önceki kayıtları silmez.

Salt okunur gerçek Chrome transport probe, resmi Steam sayfasından `login_required` durumunu başarıyla okudu. Bu hesap girişi veya tam sahip olunan oyun listesi kabul testi değildir. Gerçek hesap listesi kullanıcı Steam girişini tamamlayınca doğrulanır. Epic, resmi Legendary WebView girişini kullanır; kod kopyalama/konsol adımı kaldırıldı. Helper'ın çıkış koduna ek olarak başarılı kütüphane okuması gerekir. [Epic kanıtları](epic-login.md), [hesaplar ve kapaklar](hesaplar-kapaklar-hltb.md).

## Discord onarımı

Gerçek kurulu `%APPDATA%/Vencord/dist/renderer.js` paketinde GameLibraryBridge yoktu. Resmi Vencord kaynağına güncel eklenti eklenip TypeScript kontrolünden geçirildi ve standalone paket derlendi. Mevcut paket ve ayar dosyası yedeklendi; köprüyü içeren paket kuruldu. Data directory `%LOCALAPPDATA%/GameLibrary` olarak düzeltildi. Taşıma, yapılandırılmış eski paket profilinden normal native profile geri dönebiliyor.

Kullanıcının izniyle Discord yeniden başlatıldı. Canlı loopback eşleşmesi başarılı: **32 arkadaş, 1 ses odası**, mute/deafen, arkadaş/oda favorileri ve son konuşma yetenekleri okundu. Broadcast ses cihazı seçim adaptörü gerçek Discord sözleşmesinde kullanılabilir (`broadcast_audio: true`); güncel Spotify native düğme adaptörü de gerçek istemcide tanındı (`spotify_join: true`). Test sırasında görüşme, birlikte dinleme, yayın veya ses cihazı değiştirme komutu gönderilmedi. Go Live başlatma/kalite/FPS bu anlık istemci durumunda kullanılabilir değildi; UI bunları kapalı gösterir.

Standart Vencord otomatik güncellemesi özel köprüyü içermeyen pakete geçebildiği için bu kurulumda kapalıdır. Bunun yaşanan arızanın kesin nedeni olduğu iddia edilmez. **Köprüyü hazırla / güncelle** resmi checkout'u günceller, kendi köprümüzle derler, eski paketi yedekleyip kurulu dosyalara uygular. Eklentisiz paket mevcut kurulumun üzerine yazılmaz; dosya uygulama hatasında yedekten geri alma denenir.

## Yayın, ses ve müzik

Arkadaş tıklaması, yeni Discord `openPrivateChannel` nesne sözleşmesine göre düzeltildi ve uygulama içinde native sohbet görünümü eklendi. Yalnız seçilen arkadaşın doğrulanan birebir kanalından son 30 metin mesajı alınır; grup kanalları reddedilir. Kullanıcı Enter/Gönder seçtiğinde kendi metni gönderilir; aynı nonce/kanal/yazar/metin ve `SENT` geri okuması olmadan başarı gösterilmez. Bu çalışmada canlı mesaj gönderilmedi. [Sohbet ayrıntıları](chat.md).

Güncel sohbet paketi yedek alınarak kuruldu. Son kontrolde kullanıcı Discord ses kanalına bağlıydı; kullanıcı görüşme sonrası kendisinin yeniden başlatmasını seçti. Bu yüzden canlı oturum eski paketi kullanmaya devam ediyor. Yeni native uygulama sohbet yeteneği gelmeden eski birebir DM çağrısını göndermez ve yeniden başlatma gereğini gösterir.

- **OBS:** localhost WebSocket v5, parola doğrulama, sahne listesi/seçimi, açık başlat/durdur yayın ve kayıt kontrolleri. Parola Windows Credential Manager'da saklanır; her eylem sonrası OBS durumuyla doğrulanır. OBS bu kontrolde çalışmıyordu. Sahte sunucu kabul testleri gerçek yayın/kayıt başlatıldığı anlamına gelmez. [OBS ayrıntıları](obs.md).
- **NVIDIA Broadcast:** gerçek Windows kurulum/GPU/sanal cihaz keşfi. Bu makinede Broadcast 2.2.1.11121, RTX 4070 ve mikrofon/hoparlör/kamera bulundu. Discord sesini Broadcast'a bağlama düğmesi gerçek cihaz kataloğu ve giriş/çıkış geri okumasını kullanır. Kamera/efekt akışı veya canlı cihaz yönlendirme testi yapılmadı. [Broadcast ayrıntıları](nvidia-broadcast.md).
- **Spotify:** Discord bağlantısı ve güncel arkadaş parçası okunur, Spotify'da parça açılır. Tanınan istemcide **Birlikte dinle**, Discord'un kendi uygulama açma/Premium kontrolü yapan düğme fonksiyonuna bağlanır. İstemci sözleşmesi değişirse profil düğmesi kullanılır. Sonuç aynı arkadaşın Discord geri okumasıyla doğrulanır; bekleyen native Promise köprü heartbeat'ini durdurmaz. Canlı Premium dinleme başlatılmadı. Token erişim metotları kullanılmaz. [Spotify ayrıntıları](spotify.md).

## İsteğe bağlı ağ erişimi

**Ayarlar → Ağ erişimi** Cloudflare DNS, DoH, önceki ayarları geri alma, GoodbyeDPI başlat/durdur ve resmi paket güncellemesi sağlar. Varsayılanlar kapalıdır. Kullanıcı düğmesi olmadan DNS, süreç veya WinDivert sürücüsü etkinleştirilmez. DNS geri alma kayıtları ve çalıştırılan süreç kimliği korunan Windows alanında tutulur. Otomatik paket hazırlama yeni sürümü sonraki açık başlatma eylemine bırakır.

DNS adresleri Cloudflare'ın resmi tanımıdır; indirilen araç resmi `ValdikSS/GoodbyeDPI` kararlı release kanalındandır. DNS ayrı bir yazılım paketi değildir. 0.2.2 ZIP'i yayıncı SHA-256 alanı sunmadığı için bağımsız yayıncı imzası doğrulandı denmez. [Ağ modülü, güvenlik ve kabul sınırları](network-tools.md).

Canlı DNS/UAC/GoodbyeDPI/WinDivert ve uygulama kapanışı gözcüsü bu çalışmada etkinleştirilmedi. Ağ ekranının gerçek salt okunur bağdaştırıcı keşfi arayüzde doğrulandı.

## Görseller

Game Library, Steam, Epic, Riot, Discord, Spotify, NVIDIA, OBS, Cloudflare ve GoodbyeDPI simgeleri ChatGPT'nin yerleşik `imagegen` sistemiyle üretildi. Kaynaklar `assets/logo-sources/*.png`, kullanılan promptların tamamı [prompts.json](../assets/logo-sources/prompts.json), optimize native PNG/Windows ICO dosyaları `apps/native/ui/logos/` içindedir. GoodbyeDPI için kullanılan kalkan resmi upstream logosu olduğu iddiasını taşımaz. Ana pencere ve tepsi simgesi yeni uygulama görselini kullanır.

## Doğrulama sınırları

Native 47 birim testi, ağ harness'i 7 test ve 58 PowerShell assertion, Steam core 13 test, toplam 70 Node köprü/tarayıcı testi ve resmi Vencord TypeScript/standalone derlemesi geçti. Çalışma alanının tüm hedeflerinde Clippy uyarısız geçti. Native arayüzde sohbet, kaynak girişleri, entegrasyon ve ağ paneli görsel olarak incelendi; sohbet ekran görüntüsü açıkça işaretlenen örnek veridir. Son release Windows executable kaynakla birlikte yeniden derlendi.

Canlı Discord verisi, gerçek Broadcast metadata'sı, ayrı Steam tarayıcı taşıması ve network bağdaştırıcı keşfi doğrulandı. Steam/Epic tam hesap eşitlemesi kullanıcı girişini; OBS gerçek bağlantısı açık WebSocket sunucusunu; Spotify birlikte dinleme Premium oturumunu gerektirir. Önceki performans raporları bu yeni derlemenin ölçümü sayılmaz.

Oturumun araç kataloğunda `snyk_code_scan` bulunmadı; Snyk CLI ve aranabilir Snyk eklentisi de yoktu. Zorunlu Snyk taraması tamamlanmadı. Testler ve Clippy güvenlik taramasının yerine geçmez.
