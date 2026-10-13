# NVIDIA Broadcast bağlantısı

Game Library, kurulu NVIDIA Broadcast uygulamasını bulup açar ve Windows'ta kayıtlı Broadcast mikrofon, hoparlör ve kamera cihazlarını listeler. Ses için aktif MMDevice uçları; kamera için DirectShow cihaz moniker'ları kullanılır. Kamera/mikrofon akışı açılmaz, varsayılan Windows cihazı değiştirilmez. Keşif yalnızca kullanıcı yenilediğinde çalıştırılmalıdır.

GPU bilgisi Windows görüntü sürücüsünden okunur. NVIDIA PCI üretici kimliği ve sürücü adındaki `RTX` sözcüğü bir RTX adayı gösterir; bellek, sürücü ve tüm özelliklerin uyumunu kanıtlamaz. NVIDIA'nın temel gereksinimi RTX 2060 / Quadro RTX 3000 / TITAN RTX veya üzeri ile 8 GB RAM'dir. Son uyumu NVIDIA yükleyicisi/uygulaması doğrular. [NVIDIA Broadcast FAQ](https://www.nvidia.com/en-us/geforce/broadcasting/broadcast-app/faq/)

Broadcast'ta fiziksel mikrofon/kamera seçilip efektler ayarlandıktan sonra Discord veya OBS'ta `Microphone (NVIDIA Broadcast)`, `Speakers (NVIDIA Broadcast)` ya da `Camera (NVIDIA Broadcast)` seçilmelidir. NVIDIA, uygulamanın ikinci gürültü giderme filtresini kapatmayı önerir. [Resmi kurulum rehberi](https://www.nvidia.com/en-us/geforce/guides/broadcast-app-setup-guide/)

Discord köprüsündeki `broadcast_audio` işlemi, kullanıcının açık düğme isteğiyle Discord'un gerçekten listelediği Broadcast mikrofonu/hoparlörünü seçebilir. Kamera ayarını değiştirmez. Varsayılan, devre dışı veya adı belirsiz cihazlar seçilmez; iki seçili cihaz kimliği MediaEngineStore'dan geri okunmadan işlem başarılı gösterilmez. API tanınmıyorsa kontrol kapanır. [Resmi Vencord MediaEngineStore tipleri](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/MediaEngineStore.d.ts)

Audio action'ları Vencord'da ayrıca tiplenmediğinden adapter, Discord'un kendi istemcisindeki `AUDIO_SET_INPUT_DEVICE` / `AUDIO_SET_OUTPUT_DEVICE` ID dispatch sözleşmesini doğrular. [İncelenen Discord istemci asset'i](https://discord.com/assets/web.b9c066df3452b7ea.js)

## Native API

- `broadcast::inspect() -> Report`: kurulum, sürüm, GPU kanıtı, gerçek cihaz kimlikleri ve keşif hataları.
- `Report::summary() -> String`: entegrasyon ekranına uygun Türkçe durum.
- `Report::device(DeviceKind) -> Option<&Device>`: tür bazında gözlenen sanal cihaz.
- `broadcast::open() -> Result<String, String>`: yeni keşifle doğrulanan kurulu EXE'yi açar.
- `broadcast::open_download()` / `open_setup()`: resmi NVIDIA sayfalarını açar.

Kurulu EXE, Program Files ve Windows kurulum kayıtlarından keşfedilir; UI'dan keyfi çalıştırılabilir dosya yolu alınmaz. `Report.devices` listesinin dolu olması Broadcast efekti, ses/kamera akışı veya Discord yönlendirmesi için başarı kanıtı değildir. COM/cihaz keşfi hataları boş listeden ayrı raporlanır.

Yeni NVIDIA sürümleri `Settings → AI Agent Resources` altında yerel MCP bağlantısı sunabilir. Adres ve araç şeması kurulu uygulamadan alınmalıdır; Game Library bu sürümde endpoint tahmin etmez ve efektleri bu API ile yönetmez. [NVIDIA MCP açıklaması](https://www.nvidia.com/en-us/geforce/broadcasting/broadcast-app/faq/)

## Doğrulama

```powershell
cargo test -p game-library-native --example broadcast-probe
cargo run -p game-library-native --example broadcast-probe
```

İkinci komut yalnızca metadata raporu basar. Canlı kabul: Broadcast'u aç; fiziksel cihazları seç; Game Library'de yenile; Discord/OBS'ta gözlenen sanal cihazı seç; gerçek görüşme/OBS kaydı ile efekt ve ses/kamera akışını doğrula. Mikrofon/hoparlör Windows'ta devre dışıysa aktif uç listesinde görünmez.

13 Ekim 2026 doğrulaması: dört dar test geçti. Bu bilgisayarda Broadcast `2.2.1.11121`, RTX 4070 ve üç Broadcast sanal cihazı gerçek Windows kimlikleriyle bulundu; keşif hatası yoktu. Bu kontrol ses/kamera akışı ya da efekt kalitesi testi yapmadı.

Discord yardımcı modülünün 12 testi ve TypeScript strict kontrolü geçti. Testler mock store/action kullanır; canlı Discord ses cihazı değiştirilmedi. Köprü testleri mevcut `bridge-test-loader.mjs` ile Node 24 kullanılarak çalıştırıldı:

```powershell
node --import ./scripts/bridge-test-loader.mjs --test ./scripts/bridge-broadcast.test.mjs
```
