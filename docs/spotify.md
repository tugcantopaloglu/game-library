# Spotify ve Discord birlikte dinleme

Bu entegrasyon Discord'un mevcut Spotify bağlantısını ve izin verdiği müzik etkinliğini okur. Game Library kendi Spotify oturumu açmaz; Discord veya Spotify erişim tokenı okumaz, dosyaya kaydetmez ya da köprüye taşımaz.

## Kullanım

1. Discord > Kullanıcı ayarları > Bağlantılar > Spotify üzerinden hesabını bağla.
2. Arkadaşlarının parçanı görmesini istiyorsan Spotify durumunu göster seçeneğini aç.
3. Game Library'deki Spotify parça düğmesi yalnızca seçilen güncel parça kimliği için `spotify:track:<id>` bağlantısını açar.
4. Uyumlu istemcide arkadaşın satırındaki **Birlikte dinle** düğmesini kullan. İstemci değişmişse profil düğmesi Discord'un kendi seçeneğini açar. Dinleyicinin Spotify Premium hesabı gerekir. Bu işlem otomatik davet veya mesaj göndermez.

Spotify'ın resmi desteği birlikte dinlemenin ses sohbeti sırasında kullanılamadığını belirtiyor. Discord ayrıca mikrofon 30 saniye etkin biçimde yayın yaparsa Spotify'ın otomatik durakladığını açıklıyor. Game Library bu koşulları değiştirmez.

## İstemci uyumluluğu

`spotify.ts` yalnızca `SpotifyStore.hasConnectedAccount`, `getTrack`, `getSyncingWith`, `wasAutoPaused` ve arkadaşların `PresenceStore.getActivities` verilerini okur. Etkinliğin türü, sağlayıcı adı, halka açık parça kimliği, başlangıcı ve bitişi doğrulanır. Süresi dolmuş etkinlik birlikte dinleme hedefi olarak kullanılamaz. İstemci erişim metodu bulunamazsa durum bilinmiyor olarak gösterilir.

Doğrudan katılma, yalnızca gerçek Discord katılma fonksiyonunun argüman sözleşmesi ayrıca doğrulanıp `SpotifyProvider.join` adaptörü sağlanırsa etkinleşir. 13 Ekim 2026 araştırmasında Discord'un herkese açık `web.b19183c9af96925c.js` dosyasındaki native Listen Along düğme fonksiyonu doğrulandı. `listenAlongAdapter` yalnızca bu fonksiyonun üç argümanlı, Spotify uygulamasını açan ve gerçek Premium kontrolünden geçen tanınmış kod yapısına eşleşir. Bu yapı değişirse doğrudan katılma kapanır ve profil düğmesi kullanılır. Düşük seviyeli `ACTIVITY_SYNC` dispatcher'ı doğrudan çağrılmaz. Katılım sonucu `getSyncingWith().userId` ile aynı arkadaştan geri okunur; native Spotify açılışı 30 saniye bekleyebildiği için 45 saniyelik doğrulama süresi vardır. Native Promise köprü poll döngüsünü bekletmez; hesap değişimi, yeniden bağlantı veya kapatma sonrası gecikmiş sonuç durumunu değiştiremez.

## Modül bağlantısı

`apps/native/src/spotify.rs`: `valid_track_id`, `track_uri`, `open_track`, `open_app`. Yeni bağımlılık gerekmez; mevcut `platform_windows::shell::open_target` kullanılır. Sadece kullanıcı düğmesine basınca çağrılır.

`integrations/vencord/gameLibraryBridge.desktop/spotify.ts`: `SpotifyControls`, `SpotifyProvider`, `SpotifyStoreReader`, `spotifyActivity`, `spotifyTrack`, `validTrackId`, `listenAlongAdapter`.

Köprü `controls` bölümüne `spotify: spotifyControls.state(provider)` koyar. `SpotifyStore`, `PresenceStore`, `RelationshipStore`, `UserStore` değişince `controls` önbelleğini geçersizleştirir. `spotify_track` işlemi `{trackId, userId?}`, `spotify_profile` ve varsa `spotify_join` işlemleri `{userId}` alır. Her poll turunda `spotifyControls.observe(provider)` çağrılır; yeniden bağlantıda ve eklenti kapanınca `spotifyControls.clear()` çağrılır. Profil callback'i Vencord'un `@utils/discord.openUserProfile` yardımcı fonksiyonuna bağlanır. URI callback'i `VencordNative.native.openExternal` kullanır.

## Bağımsız Spotify Web API

Bağımsız Spotify playback kontrolü bu değişiklikte yoktur. Böyle bir özellik Spotify Developer Dashboard'da kullanıcıya ait uygulama/client ID ve kullanıcı OAuth onayı gerektirir. Masaüstünde client secret güvenle saklanamadığından Spotify Authorization Code + PKCE önerir. Playback değiştirmek `user-modify-playback-state` izni, etkin Spotify cihazı ve Premium hesabı gerektirir. Bunlar sağlanmadan oynat/duraklat/senkronizasyon kontrolleri etkin gösterilmemelidir.

## Doğrulama

Node.js 22.13 veya daha yeni bir sürümle: `node --import ./scripts/bridge-test-loader.mjs --test scripts/bridge-spotify.test.mjs`. Loader `register` ve asenkron module hooks kullanır; sistem Node.js 22.14 üzerinde tüm 17 Spotify kontrolü geçti.

Testler token erişim metotlarının kullanılmamasını, dışarı taşınan metinlerin sınırlarını, yalnızca güncel arkadaş etkinliklerine erişimi, seçilen parçanın değişmesini, bilinmeyen istemci yeteneklerini, kesin katılma geri okumasını ve zaman aşımını kapsar. Canlı Premium oturumu ve gerçek Discord katılma aksiyonu bu testlerle doğrulanmış sayılmaz.

## Birincil kaynaklar (13 Ekim 2026)

- [Spotify: Discord ve Spotify](https://support.spotify.com/us/article/discord-and-spotify/)
- [Discord: Spotify bağlantısı](https://support.discord.com/hc/en-us/articles/360000167212-Discord-Spotify-Connection)
- [Vencord: SpotifyStore türleri](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/SpotifyStore.d.ts)
- [Vencord: Spotify Controls kaynak kodu](https://github.com/Vendicated/Vencord/blob/main/src/plugins/spotifyControls/SpotifyStore.ts)
- [Spotify: URI ve ID biçimleri](https://developer.spotify.com/documentation/web-api/concepts/spotify-uris-ids)
- [Spotify: Authorization Code with PKCE](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow)
- [Spotify: Start/Resume Playback](https://developer.spotify.com/documentation/web-api/reference/start-a-users-playback)
- [Discord'un herkese açık istemci kaynağı](https://discord.com/assets/web.b19183c9af96925c.js): native wrapper modül 533207; kullanıcı arayüzü çağrısı modül 353411; Premium/device yardımcıları modül 981616. İstemci fonksiyon kodu sadece okunarak incelendi; kullanıcı oturumuna erişilmedi veya dinleme işlemi başlatılmadı.

Snyk `snyk_code_scan` aracı bu oturumun araç kataloğunda bulunamadı. Bu yüzden Snyk taraması çalıştırılamadı; yerel testler güvenlik taramasının yerine geçmez.
