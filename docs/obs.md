# OBS bağlantısı

Game Library aynı bilgisayardaki OBS Studio'nun WebSocket v5 sunucusuna bağlanır.
OBS'de **Araçlar → WebSocket Sunucu Ayarları → WebSocket sunucusunu etkinleştir**
seçeneğini açın. Varsayılan port **4455**; farklı bir port ayarladıysanız Game Library'de
aynı portu girin. Kimlik doğrulama açıksa sunucu parolasını Game Library'nin OBS alanına
girin. Parola Windows Credential Manager'da port başına saklanır; oyun veritabanına
veya günlük dosyasına yazılmaz. Boş parola kaydetmek kayıtlı parolayı siler.

**Bağlan / Yenile** gerçek OBS sürümü, sahneler, mevcut program sahnesi, yayın ve kayıt
durumunu okur. **Sahneyi uygula**, **Yayını başlat / durdur** ve **Kaydı başlat / durdur**
yalnızca ilgili kullanıcı işlemiyle çalışır. Başarılı komut yanıtından sonra durum
yeniden okunur. OBS henüz hedef duruma geçmediyse arayüz doğrulama beklediğini bildirir;
komut otomatik tekrarlanmaz. Durum görünümü son manuel sorgunun sonucudur. OBS'de başka
bir değişiklik yaptığınızda **Yenile** ile güncellenir.

Bağlantı her manuel işlem için açılıp kapanır; arka planda sürekli sorgu, timer veya
OBS'yi başlatan işlem yoktur. Sunucu bağlantısı yalnızca `127.0.0.1` adresine yapılır.
Kendi OBS yayın anahtarınızı, yayın hedefinizi ve kayıt ayarlarınızı OBS üzerinden
ayarlayın; bu entegrasyon bu ayarları değiştirmez.

Geliştirici API'si:

```rust
// Worker thread üzerinden çağırın. UI thread üzerinde ağ işlemi çalıştırmayın.
let password = platform_windows::credentials::obs_password(port)?;
let state = obs::snapshot(port, password.as_deref())?;

// Yalnızca kullanıcının ilgili butonu seçmesiyle:
let result = obs::perform(port, password.as_deref(), obs::Action::StartRecording)?;
// result.snapshot gözlenen durumu, result.verified hedef durum doğrulamasını içerir.
```

RPC yanıtlarında istek kimliği, istek türü ve başarı kodu birlikte doğrulanır.
Bağlantı 2 saniye, WebSocket görüşmesi ve her RPC 3 saniye ile sınırlıdır. Kısmi frame'ler
mutlak süre sınırını uzatamaz; mesaj/frame sınırı 1 MiB'dır. Hiçbir event aboneliği açılmaz.
Parola challenge/response için SHA-256 ve Base64 kullanılır; doğrudan gönderilmez.

Kaynaklar: [OBS WebSocket projesi](https://github.com/obsproject/obs-websocket),
[resmi v5 protokolü](https://github.com/obsproject/obs-websocket/blob/master/docs/generated/protocol.md),
[tungstenite belgeleri](https://docs.rs/tungstenite/0.30.0/tungstenite/).

Doğrulama: yerel sahte WebSocket sunucusuyla parolalı oturum, yalnızca okunabilir durum
sorgusu, sahne/yayın/kayıt komutları ve işlem sonrası doğrulama, ret yanıtı, eşleşmeyen
istek kimliği, bozuk durum alanları ve süre/boyut sınırları test edilir. Gerçek OBS ile
kabul testi sunucunun açık olduğu kullanıcı ortamında yapılmalıdır.
