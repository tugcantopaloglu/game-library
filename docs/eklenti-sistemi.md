# Eklentiler

Ayarlar → Eklentiler ekranından `extension.json` içeren bir klasör kurulur. Paketler varsayılan olarak kapalıdır. Etkinleştir, Klasör ekle, Çalıştır, Devre dışı bırak ve Kaldır işlemleri vardır. Kurulu paketler `%LOCALAPPDATA%/GameLibrary/extensions/<id>/` altında tutulur.

Eklenti yalnız kullanıcı Çalıştır'ı seçince ayrı executable süreçte açılır. JSON-RPC isteği stdin'e yazılır; tek yanıt stdout'tan alınır. Normal kullanımda eklenti süreci açık kalmaz. 15 saniye zaman aşımı, 4 MiB yanıt ve 64 KiB istek sınırı vardır. Windows Job Object kapanınca yardımcı süreçler de kapatılır. Çökme ve geçersiz yanıt mevcut oyunları kaldırmaz.

Eklentiler güvenilen yerel paketler içindir. Windows kullanıcı yetkileriyle çalışırlar; bildirimin izin listesi bir güvenlik sandbox'ı değildir. DLL ve arayüze enjekte edilen kod kullanılmaz.

## Paket sözleşmesi

```json
{
  "id": "local-library",
  "name": "Yerel oyun listesi",
  "version": "1.0.0",
  "protocolVersion": 1,
  "entrypoint": "local-library.exe",
  "args": ["--extension-example"],
  "capabilities": ["games.discover"],
  "permissions": ["filesystem.read"]
}
```

Kimlik en fazla 64 küçük harf, rakam, nokta veya tire içerir. Entrypoint paketin içinde bir `.exe` olmalı. Üst klasöre çıkış, symlink/junction ve 128 MiB üstü paketler reddedilir. Kurulan paketin getirdiği etkinlik veya klasör tercihleri içe alınmaz.

İstek `{"jsonrpc":"2.0","id":1,"method":"games.discover","params":{"protocolVersion":1,"pluginId":"local-library","roots":["C:/Games"]}}` biçimindedir. Yanıt aynı JSON-RPC sürümü ve istek kimliğiyle `result={protocolVersion:1,pluginId:"local-library",games:[...]}` döndürür.

Her oyun `id`, `name`, `executable` içerir. Executable seçilen klasörlerin içinde, var olan `.exe` dosyası olmalı. En fazla 2.000 kayıt kabul edilir. Kabuk komutu ve eklentiden gelen başlatma argümanları çalıştırılmaz. Kayıtlar sabit `extension:<eklenti>:<oyun>` kimliğiyle kütüphaneye girer; kullanıcı başlığı, notları ve favorileri yeniden çalıştırmada korunur. Tam ve geçerli yanıt olmadan önceki kurulum durumu değiştirilmez.

`metadata.lookup` ikinci yetenektir. İlk 100 oyun kimliği/adı/kaynağı iletilir; yanıt `metadata=[{id,artworkUrl}]` içerir. Yalnız istenen oyunlara ait HTTPS kapak adresleri kabul edilir. İndirme ayrıca mevcut kapak sağlayıcılarının güvenilir alan adı ve dosya sınırlarına tabidir. Tüm kütüphaneyi sayfalayan metadata protokolü ve eklenti mağazası henüz eklenmedi.

## Çalışan örnek

Örnek eklenti ekle düğmesi uygulamanın içinde bulunan JSON-RPC yardımcı modunu paketler. Sonra seçilen oyun klasöründe şu dosya oluşturulur:

```json
[
  {"id":"my-game","name":"Oyunum","executable":"C:/Games/MyGame/game.exe"}
]
```

Dosya adı `games.json` olmalı. Klasörü eklentiye ekleyip etkinleştir ve Çalıştır'ı seç. Bu örnek executable başlatmaz; yalnız kütüphaneye kaydeder.

Geliştirici kontrolleri için `--extension-command install|enable|disable|root|run|list|remove`, `--extension-id`, `--folder`, `--data-dir` ve `--output` seçenekleri vardır. `--create-extension-example <klasör>` yeni örnek paket üretir.
