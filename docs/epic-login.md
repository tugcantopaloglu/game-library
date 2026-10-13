# Epic hesabına kod kopyalamadan giriş

Legendary'nin resmi Windows yürütülebilir dosyası Epic giriş penceresini açabilir ve Epic'in kısa ömürlü exchange code callback'ini kendisi alır. Kullanıcının API anahtarı veya kod kopyalaması gerekmez. Kullanıcı hesabına giriş yapma, iki aşamalı doğrulama ve Epic'in onaylarını kendi penceresinde tamamlar.

## Doğrulanan davranış

- Resmi README, önceden derlenen Windows sürümlerinde 0.20.14 ve üstü için varsayılan `legendary auth` komutunun Epic giriş penceresini açtığını açıklıyor.
- 0.21.1 `release.yml`, `build-base.yml` iş akışını kullanıyor. Windows derlemesi WebView bağımlılıklarını `uv sync --extra webview --no-install-local --inexact` ile ekliyor.
- 0.21.1 `cli.py`, WebView kullanılabilirken `do_webview_login(callback_code=self.core.auth_ex_token, ...)` çağırıyor.
- `webview_login.py`, Epic'in `requestexchangecodesignin` callback'ini alıp doğrudan oturum yardımcısına geçiriyor. Game Library'nin bu kodu, parolayı veya tokenı işlemesine gerek yok.
- `--disable-webview`, bu pencereyi kapatıp kullanıcıdan authorizationCode kopyalaması isteyen terminal akışını zorunlu tutuyor. Önceki bağlantı akışındaki ek adımın sebebi bu parametreydi.

## Native çağrı sözleşmesi

`accounts::login_epic` mevcut doğrulanmış Windows helper'ını, ayrı `LEGENDARY_CONFIG_PATH` ile yalnızca `auth` argümanıyla çalıştırır. `--disable-webview` ve `CREATE_NEW_CONSOLE` kullanılmaz. `CREATE_NO_WINDOW` ve null stdin/stdout/stderr terminal penceresini/loglarını kapatır; Epic'in WebView penceresi ayrı görünür kalır. `--import`, `--sid`, `--token` ve `--code` kullanılmaz.

Başlangıçta helper'ın açılması oturumun başarılı olduğu anlamına gelmez. Legendary, kullanıcı pencereyi kapattığında veya WebView girişini tamamlayamadığında hata kaydı yazıp sıfır çıkış koduyla dönebilir. Giriş başarısı ayrı yardımcının oturum dosyası oluşması ve mevcut `legendary list --json --include-non-installable` çağrısının başarılı yanıt vermesiyle doğrulanır. Oyun eşitleme bu okumadan sonra yapılır.

WebView2 eksikse veya Legendary'nin genel WebView kapatma anahtarı etkinse helper tarayıcı/kod akışına dönebilir. Null stdin elle kod adımını bekletmez; uygulama kullanıcıya giriş penceresinin açılamadığını gösterir. Kod kopyalaması istenmez. Başarısız işlem önceki kütüphaneyi korur.

## Bu makinede salt okunur kontrol (13 Ekim 2026)

Yerel helper `legendary version "0.21.1"` çıktısını verdi; `auth --help` gömülü WebView seçeneğini doğruladı. Yerel dosyanın SHA-256 özeti `6be77857dc0a6dd33ea7442ce8b5291944ec6da53757f81ca95b57ce794baf5b`, resmi `legendary_windows_x64.exe` release digest'iyle eşleşti. Resmi son release de 0.21.1 (8 Eylül 2026). Legendary'nin genel sürüm API'si `webview_killswitch: false` döndürdü. Microsoft Edge WebView2 çalışma zamanı 155.0.4283.45 kayıtlı.

Canlı giriş penceresi açılmadı; hesap/parola/token okunmadı, Epic launcher oturumu içe aktarılmadı ve kullanıcı adına onay verilmedi. Başarılı hesap girişinin kabulü kullanıcının kendi Epic girişini tamamlaması sonrasında yapılır.

## Birincil kaynaklar

- [Legendary README, Quickstart](https://github.com/legendary-gl/legendary/tree/0.21.1#quickstart)
- [0.21.1 Windows build iş akışı](https://github.com/legendary-gl/legendary/blob/0.21.1/.github/workflows/build-base.yml)
- [0.21.1 release iş akışı](https://github.com/legendary-gl/legendary/blob/0.21.1/.github/workflows/release.yml)
- [0.21.1 auth uygulaması](https://github.com/legendary-gl/legendary/blob/0.21.1/legendary/cli.py)
- [0.21.1 WebView giriş callback'i](https://github.com/legendary-gl/legendary/blob/0.21.1/legendary/utils/webview_login.py)
- [0.21.1 sürümü](https://github.com/legendary-gl/legendary/releases/tag/0.21.1)
