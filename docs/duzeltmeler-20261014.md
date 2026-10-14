# 14 Ekim 2026 düzeltmeleri

Kullanıcı yayın panelinin, Discord sohbetinin ve tam Steam/Epic hesap listesinin çalışmadığını bildirdi. Önceki tamamlanma özeti bu özelliklerin canlı kabulünü olduğundan ileri göstermişti. Bu rapor kaynak kodu, üretilen dosya ve gerçek oturum sonucunu ayrı tutar.

Kontrol edilen eski `game-library-native.exe` ve `game-library-native-next.exe` aynı SHA-256 önekine sahipti (`54ef034a1ceafb02`). İkisinde de `chat_open` yoktu. Son sohbet/entegrasyon commitinin kaynak kodu bu executable'larda bulunmuyordu.

Discord adaptörlerinin biçime bağlı kontrolleri gevşetildi; bilinmeyen birebir açma sözleşmesine string kullanıcı kimliği gönderilmez. Önceden doğrulanan birebir kanal, yeni kanal açma adaptörü bulunmasa da iç sohbet için kullanılabilir. Yayın paneli artık destek algısı başarısız olduğunda gizlenmez; başlat, durdur, çözünürlük ve FPS kontrolleri görünür kalır ve kullanılabilirliklerine göre etkinleşir.

Komut iletimi Rust'ın snapshot okuma turundan ve Vencord'un saniyelik poll'undan ayrıldı. TCP komutu renderer'daki bekleyen IPC isteğini uyandırır. Store olayları anlık snapshot gönderimini tetikler; native durum değişikliği GUI işçisine doğrudan bildirilir. Heartbeat ve eskimiş veri kontrolü korunur. Bu, zamanlayıcı beklemesini kaldırır; Discord istemcisinin gerçek işlem süresi ayrıca ölçülmelidir.

Favori arkadaşlar tüm ekranlara eklenen sağ panelde gösterilir. Göster/gizle tercihi saklanır. Profil görseli, çevrimiçi/boşta/rahatsız etmeyin durumu, paylaşılan oyun ve izinle görülebilen ses odası birlikte gösterilir. Bağlantı kesildiğinde son bilinen durum güncel diye gösterilmez. Avatarlar yalnız Discord CDN'den ayrı işçide alınır; 64×64 decode, 256 KiB dosya, 256 görsel bellek ve 256 dosya disk sınırı vardır.

Steam giriş yönlendirmesi sırasında yok olan tarayıcı execution context'i için yeniden bağlanma eklendi. Store tokenı/yanıtı kullanılamıyorsa aynı özel tarayıcı oturumundaki kendi Steam Community oyun listesine geçilir; başka hesabın listesi reddedilir. Eksik/geçersiz listeler kaydedilmez. Epic listesi `--force-refresh` ile alınır. Epic komut satırı girişi artık listeyi SQLite'a kaydeder. Başarılı elle eşitleme sonrasında arama, kurulu/favori/koleksiyon filtreleri temizlenip ilgili kaynağın oyunları gösterilir.

Genel yerel eklenti yöneticisi ve executable/stdio JSON-RPC host'u eklendi. Sürüm, kimlik, yetenek, paket sınırları, etkinleştirme, klasör seçimi, süreç grubu, zaman aşımı ve kütüphaneye uygulama vardır. [Protokol ve örnek paket](eklenti-sistemi.md).

Bu çalışma ortamında Discord/Vencord kurulumu ve Steam/Epic hesap oturumu bulunmadığından gerçek sohbet, yayın, hesap sayıları, profil fotoğrafı ve mikrofon gecikmesi canlı doğrulanamaz. Fixture ekranları gerçek oturum kanıtı değildir. Yeni sürümün RAM/CPU veya sekiz saat kararlılık sonucu bu kayıtla kabul edilmez.

Gerçek CDN denemesinde `cdn.discordapp.com` TLS bağlantısı hem native istemcide hem PowerShell/Python'da başarısız oldu. Güncel köprüde avatarlar Discord'un Electron ağ yolundan istenir; yalnız PNG/JPEG görsel verisi sınırlı paketle native decode işçisine aktarılır. Mevcut oturum/yol sorunu uygulamanın ayrı HTTP istemcisini etkilediğinde bu yol Discord'un kendi erişimini kullanır. Bütün yollar başarısızsa sağ panel açık bir fotoğraf yükleme uyarısı gösterir; ağ ayarları otomatik değiştirilmez. Bu ortamda gerçek Discord hesabı olmadığı için Electron yolu canlı doğrulanamadı.

Legendary `--force-refresh` ve liste seçenekleri [upstream CLI](https://github.com/legendary-gl/legendary/blob/master/legendary/cli.py) üzerinden kontrol edildi; resmi Windows yardımcı dosyası yayımlanan SHA-256 ile indirilip seçenekleri gerçek executable'dan okundu.
