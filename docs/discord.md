# Discord entegrasyonu

## Erişim modeli

Discord bağlantısı tek bir sınırsız API değildir. Arkadaş listesi, sunucu üyeleri, ses odaları, mesaj gönderimi ve Go Live kontrolü farklı yetkilere sahip. Her sağlayıcı desteklediği işlemleri bildirecek; arayüz yalnızca kullanılabilir özellikleri açacak.

## Yetenek tablosu

| İstek | Yol | Durum ve sınır |
| --- | --- | --- |
| Hesap bağlama | Resmi OAuth / Social SDK | Uygulama kaydı ve kullanıcı yetkilendirmesi gerekir |
| Seçili arkadaşların durumu | Social SDK arkadaş/presence sistemi | Launcher kullanımına uygunluk ve alınabilen veri doğrulanacak |
| Kullanıcı adına DM | Social SDK iletişim yetenekleri | Erişim, kullanıcı ayarları ve oran sınırlarına bağlı |
| Sunucudaki ses odası doluluğu | Yetkili sunucu botu + Gateway voice-state olayları | Botun eriştiği sunucular ve kanallar için |
| Sunucu üyelerinin aktivitesi | Bot + presence intent | Erişim şartları ve kullanıcı görünürlüğüyle sınırlı |
| Bot olarak mesaj | Bot API | Gönderen bot olarak açıkça gösterilir |
| Kanal seçme, mute/deafen | Onaylı RPC veya ayrı deneysel istemci köprüsü | RPC yetkileri herkese açık değil |
| Go Live kaynak, kalite/FPS, başlat/durdur | Teknik deneme gerektiren istemci köprüsü | Belgelenmiş genel API desteği doğrulanmış değil |
| Vencord kurulum/onarım | Resmi Vencord installer | Ayrı ve isteğe bağlı modül |

Metin kanalında kimin o anda ekranı açık tuttuğunu gösteren genel bir üyelik durumu yok. Ses kanalında üyelik/doluluk izlenebilir. Görünmez kullanıcıyı çevrimiçi olarak tespit etmeye yönelik bir özellik olmayacak; erişilemeyen veri boş veya bilinmiyor olarak gösterilecek.

## Social SDK denemesi

Presence için `openid sdk.social_layer_presence`, iletişim için `openid sdk.social_layer` kapsamları belgelenmiş durumda. Hesap bağlama ve arkadaş listesi erişimi aynı şey değildir; yalnızca `identify` kapsamı bu ihtiyaçları karşılamaz. [Social SDK OAuth kapsamları](https://docs.discord.com/developers/discord-social-sdk/core-concepts/oauth2-scopes).

SDK mesajlaşma sunuyor, fakat belgeler oyun entegrasyonları üzerinden tanımlanıyor. Çok sayıda üçüncü taraf oyunu başlatan bağımsız bir kütüphanenin kullanımına uygunluğu bu kaynaklarla kesinleşmiyor. Uygulama kaydı ve SDK denemesiyle doğrulanacak; dağıtım onayı varsayılmayacak.

İletişim için geliştirme oranları mevcut; ölçekli dağıtımda ek erişim incelemesi var. Arkadaş verilerinin kopyalanıp kalıcı saklanmaması ve bağlantı kaldırıldığında temizlenmesi isteniyor. SDK devre dışı veya erişim reddedilmişken kütüphane çalışmaya devam edecek. [İletişim özellikleri ve erişim koşulları](https://docs.discord.com/developers/discord-social-sdk/core-concepts/communication-features).

DM geçmişi tam Discord arşivi değildir: belgelenmiş erişim en fazla 200 mesaj/72 saat ve iki kişinin de ilgili oyunu oynamış olması gibi sınırlara sahip. Kullanıcıların oyun içi DM tercihleri ayrıca uygulanır. Bu yüzden ürün ilk aşamada sınırsız sohbet istemcisi sözü vermeyecek. [DM geliştirme kılavuzu](https://docs.discord.com/developers/discord-social-sdk/development-guides/sending-direct-messages).

Resmi kullanıcı adına mesaj erişimi sağlanamıyorsa kişinin Discord görüşmesini açan bir kısayol sunulur. Botla mesaj göndermek kullanıcı hesabından mesaj göndermek yerine geçirilmez. Discord hesabının normal kullanıcı tokenı istenmez veya dosyalardan çıkarılmaz. OAuth/bot API dışında normal hesap otomasyonu Discord tarafından yasaklanmış durumda. [Self-bot açıklaması](https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots).

## Sunucu ve oda takibi

İzlenen ses kanalları için `GUILD_VOICE_STATES`, sunucu üyesi aktivitesi için gerektiğinde `GUILD_PRESENCES` kullanılacak. Presence ayrıcalıklı intent olduğundan erişim şartları kontrol edilecek. Güncel kurallar eski sunucu sayısı eşiklerine göre sabitlenmeyecek. [Gateway ve intents](https://docs.discord.com/developers/events/gateway).

Bot yalnızca bulunduğu ve izinli olduğu sunucular hakkında veri sağlar; kullanıcının bütün sunucuları kendiliğinden erişilebilir olmaz. OAuth sunucu listesi, bütün kanal üyelerine erişim sağlamaz. İlk kişisel denemede kullanıcının kendi botu düşünülebilir; dağıtımda bot tokenı istemcilere gömülmez, gerekiyorsa ayrı servis kullanılır.

Olaylar tek bağlantıda işlenecek; her kişi için ayrı timer açılmayacak. İlk snapshot ve reconnect sonrasındaki resync tamamlanana kadar oda doluluğu bilinmiyor gösterilecek. Bağlantı koptuğunda son veri çevrimiçiymiş gibi kalmayacak.

## Yayın kontrolü

Belgelenmiş RPC komutları arasında ses ayarları ve kanal seçme bulunuyor. Bunun Go Live yayınını başlatma, kaynak değiştirme veya kalite ayarı sunan genel bir API olduğu sonucu çıkarılamaz. Birçok RPC ve voice OAuth kapsamı onaylı partnerlere ayrılmış. [RPC komutları](https://docs.discord.com/developers/topics/rpc), [OAuth erişim sınırları](https://docs.discord.com/developers/topics/oauth2).

İlk deneme hangi yayın işlemlerinin istendiğini ve her birinin mümkün olup olmadığını ayrı kaydedecek: başlat/durdur, oyun/pencere seçimi, çözünürlük/FPS, ses veya sahne yönetimi. Nitro ve Discord'un kendi sınırları değiştirilmez. Program videoyu yeniden yakalayıp kodlamaz; görüntü aktarımının sahibi Discord kalır.

Resmi yol yeterli değilse Vencord tarafında yerel bir companion eklentisi yalnızca istemci kontrolleri için değerlendirilebilir. Bu yolun kararlılığı Discord sürümüne bağlıdır ve resmi destek sayılmaz. Arkadaş/DM erişim engellerini aşmak için özel user-token API kullanılmaz.

Köprü gerekirse sürümlü yetenek protokolü, yerel eşleşme, sınırlı mesaj boyutu ve zaman aşımı kullanacak. Windows named pipe tarafında mevcut kullanıcı ACL'i ve istemci kimliği doğrulaması tercih edilecek. Loopback taşıması gerekirse rastgele oturum anahtarı, Origin kontrolü ve yalnızca localhost bağlama zorunlu. Discord tokenı köprüye gönderilmez.

## Vencord yöneticisi

İstenen kurulum özelliği çekirdekten ayrı tutulacak. Kurulum, onarım ve kaldırma resmi installer üzerinden planlanıyor. Sürüm ve indirme kaynağı sabitlenecek; checksum varsa kaynağıyla doğrulanacak, yoksa bağımsız doğrulama varmış gibi gösterilmeyecek. Çalışan Discord, kurulum kanalı ve geri dönüş olanağı işlemden önce kontrol edilecek.

Vencord kendi destek sayfasında istemci değişikliklerinin Discord koşullarına aykırı olduğunu belirtiyor. Bu nedenle kurulum ayrı, açıkça etkinleştirilen bir özellik olacak. Discord güncellemeleri sonrasında otomatik uyumluluk/onarım da aynı tercihe bağlı; sürekli arka plan kontrolü yerine sürüm değişikliği veya kullanıcı isteği kullanılacak. [Vencord destek sayfası](https://vencord.dev/support/), [resmi indirme](https://vencord.dev/download/), [installer kaynağı](https://github.com/Vencord/Installer).

Bu planlama aşamasında Discord'a bağlanılmadı, mesaj gönderilmedi ve istemciye herhangi bir değişiklik yapılmadı.
