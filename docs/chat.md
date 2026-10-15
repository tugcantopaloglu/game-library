# Game Library içinde Discord birebir sohbeti

Sohbet backend'i seçilen arkadaşın Discord birebir kanalını Game Library içinde göstermek ve kullanıcı Gönder düğmesine bastığında mesajını Discord'a iletmek için tasarlanmıştır. Başka konuşmaların içeriğini, Discord hesabının tokenını veya mesaj geçmişini topluca okumaz. Oturum, seçilen arkadaş ve kanal değişince önceki görünüm temizlenir.

## Yanlış grup açılmasının nedeni

Güncel Discord istemcisinin `openPrivateChannel` fonksiyonu bir seçenek nesnesi bekler: `recipientIds` yalnızca seçilen arkadaşın kimliğini içeren tek elemanlı dizi olmalıdır. Ham arkadaş kimliği string olarak gönderildiğinde nesne destructuring sonucunda recipientIds boş kalır; native istemci boş recipients ile kanal açma yoluna gider. Vencord'un eski `@utils/discord.openPrivateChannel` yardımcısı da hâlâ string gönderdiğinden bu yardımcıya geçmek sorunu çözmez.

`directMessageAdapter` güncel native sözleşmeyi doğrular ve `{recipientIds:[userId],joinCall:false,joinCallVideo:false,navigateToChannel:true}` gönderir. Var olan ve doğrulanan kanal doğrudan yeniden kullanılır. Dönen kanalın türü `1` olmalı; guild'i olmamalı, tek recipient tam seçilen arkadaş olmalı. Grup kanalına (`3`), farklı kişiye veya bilinmeyen kanala gönderim reddedilir.

## Backend sözleşmesi

`directMessage.ts` dışa aktarımları: `ChatControls`, `ChatProvider`, `directMessageAdapter`, `chatMessageAdapters`, `discordFocusAdapter`, `oneToOneChannel`, `chatMessages`, `validContent`.

Root provider bağlantıları:

- `account`: `UserStore.getCurrentUser()?.id` getter'ı.
- `friendIds`: `RelationshipStore.getFriendIDs()`.
- `dmChannelId`: `ChannelStore.getDMFromUserId(userId)`; sonuç kanal kimliğidir.
- `channel`: `ChannelStore.getChannel(channelId)`.
- `messages`: `MessageStore.getMessages(channelId)`.
- `openDm`: `directMessageAdapter(ChannelActionCreators)`.
- `fetch`, `send`: `chatMessageAdapters(MessageActions)`.

Snapshot `chat={accountId,revision,userId,channelId,messages,loading,historyLoading,canLoadOlder,unread,pending,canSend,sentNonce,canOpenDiscord,status}`. Mesajlar `{id,author,content,timestamp,own,attachments}` içerir. Hesap kimliği ve pozitif revision, kuyruğa alınmış komutların farklı oturum veya sohbeti etkilemesini önler. Yeni kanal açılırken userId seçilidir, channelId boş ve loading true olabilir. Bu durumda kesin scope ile `chat_close` kabul edilir.

Komutlar `chat_open{userId,accountId?,revision?}`, `chat_close{accountId?,revision?,userId?,channelId?}`, `chat_refresh{accountId?,revision?,userId?,channelId?}`, `chat_send{userId,channelId,content,accountId?,revision?}`. Sağlanan scope alanları mevcut seçime tam eşleşmelidir. Native UI bu alanları her komuta eklemelidir.

`state` controls snapshot'a eklenir. Her poll turunda `observe(provider)` çağrılır; true dönerse controls cache geçersizleştirilir. `MessageStore`, `ChannelStore`, `RelationshipStore` ve `UserStore` değişimleri controls cache'i geçersizleştirir. Yeniden bağlantı, eklenti kapanışı ve oturum değişimi `clear()` ile önceki sohbeti temizler.

## Okuma ve gönderim

`MessageStore` yalnız seçilen, doğrulanan birebir kanal için okunur; en yeni 30 doğrulanmış mesaj alınır. Önceki mesajlar düğmesi seçili kanalda `before` imleciyle 30 kayıt daha yükler. Görüşme en fazla 150 mesaj ve 96 KiB JSON tutar; hesap/kişi değişince bırakılır. Geçmiş başa eklenirken kaydırma konumu korunur. Snapshot JSON'u 96 KiB ile sınırlandırılır, yazar adı 64 ve görünen mesaj metni 4000 karaktere kırpılır. Bilinmeyen alanlar, embed'ler ve tokenlar dışarı taşınmaz. Mesaj başına en fazla dört ekin kimliği, adı, boyutu ve Discord CDN adresi gösterilir. URL seçili kanal ve ek kimliğine eşleşmelidir. HTTPS dışında veya başka alandaki adresler reddedilir.

Gerektiğinde native `MessageActions.fetchMessages({channelId,limit:30,skipLocalFetch:true})` çağrılır. Bu, Discord'un kendi kimlik doğrulama ve yetki yolunu kullanır; Game Library ayrı bir REST tokenı almaz. Açma ve yükleme Promise'ları poll döngüsünü engellemez. Gecikmiş sonuç farklı sohbeti dolduramaz.

Gönderim native `sendMessage(channelId,messageData,false,options)` sözleşmesini kullanır. `messageData` yalnız kullanıcı metni, boş emoji listeleri ve `tts:false` içerir. `options` yerel snowflake nonce'u ve izinli mention listelerini içerir. Mesaj 1–2000 karakter olmalı; satır sonu/sekme dışındaki kontrol karakterleri ve bozuk Unicode reddedilir.

Promise'ın tamamlanması başarı sayılmaz. MessageStore'da aynı kanal, nonce, kendi yazar kimliği ve içerik ile `state === "SENT"` görülmelidir. Optimistic `SENDING` kaydı onay değildir. Ancak doğrulanınca `sentNonce` değişir; GUI taslağı bu değişiklikten sonra temizler. Bekleyen gönderim ve aynı metne hızlı çift tıklama engellenir. Hata veya 30 saniye onay zaman aşımında otomatik tekrar gönderim yoktur.

## Discord penceresinde açma

İç sohbet native Discord penceresini öne getirmeyi gerektirmez. Ayrı Discord'da aç düğmesi için native wrapper `findByProps("focus","restore","minimize","maximize")` ve `discordFocusAdapter` kullanılabilir. Doğrulanmış wrapper imzası `restore(windowId)`, `focus(windowId,force=false)`; alt düzey DiscordNative.window.focus argüman sırası farklıdır. Önce birebir kanalın seçildiği `SelectedChannelStore.getChannelId()` ile doğrulanır. Pencere odağı başarılı varsayılmaz; destek yoksa kullanıcıya Discord'u elle açma mesajı gösterilir.

## Doğrulama ve kaynak

`node --import ./scripts/bridge-test-loader.mjs --test scripts/bridge-chat.test.mjs` (Node.js 22.15+). 22 offline test; native argüman şekli, grup reddi, hesap/selection scope, bounded mesajlar, optimistic gönderim, nonce geri okuması, çift tıklama, zaman aşımı ve geç Promise sonuçları kapsanır. Strict TypeScript kontrolü geçti. Canlı sohbet/mesaj/fetch işlemi bu araştırmada yapılmadı.

- [Discord'un herkese açık istemci kaynağı](https://discord.com/assets/web.b19183c9af96925c.js): kanal modülü 308528, kanal seçimi 730852, native pencere wrapper'ı 19575, MessageActions fetch/send sözleşmesi.
- [Vencord eski openPrivateChannel yardımcısı](https://github.com/Vendicated/Vencord/blob/main/src/utils/discord.tsx)
- [Vencord ChannelStore türleri](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/ChannelStore.d.ts)
- [Vencord MessageStore türleri](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/MessageStore.d.ts)

Snyk `snyk_code_scan` aracı bu oturumun araç kataloğunda bulunmadığından tarama çalıştırılamadı. Yerel testler bu taramanın yerine geçmez.

## 15 Ekim — geçmiş, okunmamış mesajlar ve ekler

`chat_older` normal yenilemeyle aynı hesap, kişi, kanal ve revision kontrolünden geçer. Tek seferde tek yükleme çalışır; başarısızlık sonrasında yeniden denemek mümkündür. Discord'un `fetchMessages({channelId,before,limit:30,skipLocalFetch:true})` sözleşmesi yerel istemci kaynağından doğrulandı. `ReadStateStore.getUnreadCount()` kişi satırında ve sabit sağ panelde rozet olarak gösterilir. Görüşmeyi açmak mesajları kendiliğinden okunmuş olarak işaretlemez.

PNG/JPEG ekler 8 MiB, 4096×4096 ve 64 MiB decode sınırıyla ayrı işçide çözülür; 640×360 önizleme üretilir. Yalnız seçili görüşmenin son altı eki istenir, kuyruk sekiz, önbellek en fazla 16 önizlemedir (yaklaşık 14,1 MiB RGBA). Hesap değişiminde önbellek temizlenir; eski hesapta başlamış indirme yeni hesapta yayımlanmaz. Başarısız önizlemede dosya açma düğmesi kalır. Dosya açılırken canlı snapshot'taki mesaj/ek eşleşmesi yeniden doğrulanır. Ekler diskte sohbet arşivi olarak saklanmaz. Dosya yükleme/gönderme bu sürümde Discord penceresinden yapılır.

`--chat-media-fixture` gerçek mesaj veya hesap kullanmadan görsel/dosya satırlarını gösterir. Bu fixture ve offline testler canlı hesapta uçtan uca gönderim testi değildir.

Geçmiş görünümünde Discord MessageStore düzenleme ve silmelerin kaynağıdır; silinen mesaj metni ayrı bir geçmiş haritasında tutulmaz. Yeni geçmiş/ek alanları için köprü derlemesi `20261015.1` hazırlanmalıdır.
