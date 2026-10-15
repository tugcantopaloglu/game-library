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

Kimlik harf veya rakamla başlar; en fazla 64 küçük harf, rakam, nokta veya tire içerir. Entrypoint paketin içinde bir `.exe` olmalı. Üst klasöre çıkış, symlink/junction ve 128 MiB üstü paketler reddedilir. Kurulan paketin getirdiği etkinlik veya klasör tercihleri içe alınmaz.

İstek `{"jsonrpc":"2.0","id":1,"method":"games.discover","params":{"protocolVersion":1,"pluginId":"local-library","roots":["C:/Games"]}}` biçimindedir. Yanıt aynı JSON-RPC sürümü ve istek kimliğiyle `result={protocolVersion:1,pluginId:"local-library",games:[...]}` döndürür.

Her oyun `id`, `name`, `executable` içerir. Executable seçilen klasörlerin içinde, var olan `.exe` dosyası olmalı. En fazla 2.000 kayıt kabul edilir. Kabuk komutu ve eklentiden gelen başlatma argümanları çalıştırılmaz. Kayıtlar sabit `extension:<eklenti>:<oyun>` kimliğiyle kütüphaneye girer; kullanıcı başlığı, notları ve favorileri yeniden çalıştırmada korunur. Tam ve geçerli yanıt olmadan önceki kurulum durumu değiştirilmez.

`metadata.lookup` ikinci yetenektir. Kütüphane sabit oyun kimliğiyle 100'lük sayfalara bölünür; her sayfada kimlik/ad/kaynak ve `page={after,limit:100}` iletilir; yanıt `metadata=[{id,artworkUrl}]` içerir. Yalnız istenen oyunlara ait HTTPS kapak adresleri kabul edilir. İndirme ayrıca mevcut kapak sağlayıcılarının güvenilir alan adı ve dosya sınırlarına tabidir. Başlangıçtaki en büyük kimlik üst sınırdır; başlık ve sıralama değişiklikleri sayfalamayı etkilemez. Her sayfa tümüyle doğrulanıp tek SQLite işlemiyle kaydedilir. Sonraki sayfa hata verirse tamamlanan sayfalar korunur ve yardımcı süreç hatası işlenen sayfa/oyun sayısını içerir. Eklenti mağazası yoktur.

## Çalışan örnek

Örnek eklenti ekle düğmesi uygulamanın içinde bulunan JSON-RPC yardımcı modunu paketler. Sonra seçilen oyun klasöründe şu dosya oluşturulur:

```json
[
  {"id":"my-game","name":"Oyunum","executable":"C:/Games/MyGame/game.exe"}
]
```

Dosya adı `games.json` olmalı. Klasörü eklentiye ekleyip etkinleştir ve Çalıştır'ı seç. Bu örnek executable başlatmaz; yalnız kütüphaneye kaydeder.

Geliştirici kontrolleri için `--extension-command install|update|rollback|enable|disable|root|run|list|remove`, `--extension-id`, `--folder`, `--data-dir` ve `--output` seçenekleri vardır. `--create-extension-example <klasör>` yeni örnek paket üretir.

## Sürüm güncelleme ve geri dönüş

Güncelle düğmesinde daha yeni yerel paket klasörünü seç. Kimlik aynı, SemVer sürümü daha büyük olmalı. Paket önce doğrulanıp ayrı klasöre kopyalanır; çalışan kurulum daha sonra değiştirilir. Kullanıcının etkinlik ve oyun klasörü tercihleri korunur. Yeni izin/yetenek eklenirse eklenti kapalı kalır; ekranda görünen izinler incelendikten sonra yeniden etkinleştirilir. Önceki paket `extension-backups/<id>` altında tutulur. Geri dön bu sürüme, mevcut klasör tercihlerini koruyarak geçer. Kaldır her iki paketi siler. Kütüphane favori, başlık ve notları paket değişiminden bağımsızdır.

257 oyunluk metadata testi üç sayfanın tamamını doğrular; izin artışı, aynı/eski sürüm reddi, ayarların korunması ve geri dönüş testleri bulunur. Güncelleme işlemi çevrimiçi mağaza veya otomatik paket indirme içermez.
