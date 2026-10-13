# Yayın kontrolleri — 13 Ekim 2026

## Başlatma ve kaynak seçici

Native Yayın panelindeki kaynak seçici mevcut ses odasında paylaşılabilecek ekran ve pencereleri listeler. Discord'un kendi desktop capture köprüsünden yalnızca ad/kimlik alınır; thumbnail boyutu sıfırdır, ikon ve video önizlemesi istenmez. Liste kullanıcı açtığında veya Yenile seçtiğinde okunur. 100 kayıt ve 30 saniyelik seçim geçerliliği sınırı vardır. Panel kapandığında, sayfa değiştiğinde, tepsiye geçildiğinde ve hesap/oda/bağlantı değiştiğinde katalog bırakılır.

Başlatma isteği seçilmiş kaynak listesinin sürümü ve mevcut ses odasıyla eşleşmelidir. Kaynak uygulanmadan önce yeniden doğrulanır; kaybolmuş veya başlığı değişmiş pencere paylaşılmaz. Uygulama başka odaya otomatik katılmaz. Discord'un `STREAM_START` action sözleşmesi tanınırsa başlatma yeteneği açılır. Seçilen kaynakla gerçek aktif yayın geri okunmadan başarı gösterilmez.

Medya action'ı `desktopSettings`, `qualityOptions` ve `context` alanlarını kullanır; normalize edilmiş MediaEngineStore okuma nesnesi doğrudan action'a verilmez. İstemci sözleşmesi tanınmazsa ilgili kontrol kapanır. Başlatma ilk aşamada yayın sesi kapalı olarak yapılır.

Kaynaklar: [Discord istemci kodu kopyası](https://github.com/Discord-Datamining/Discord-Datamining/blob/master/current.js), [Vencord MediaEngineStore tipleri](https://github.com/Vendicated/Vencord/blob/main/packages/discord-types/src/stores/MediaEngineStore.d.ts), [Electron desktopCapturer](https://www.electronjs.org/docs/latest/api/desktop-capturer). İstemci kodu kopyası canlı kurulumun sürüm uyumunun yerine geçmez.

Kısa kontrol: sekiz köprü yayın testi geçti. Sınırlandırılmış kaynak metadata'sı, kapatılan seçicinin geciken cevabı, başlatmanın gerçek durumla doğrulanması, kaybolan pencere/hesap değişimi ve action biçimi kontrol edildi. Canlı Discord oturumu bulunmadığından gerçek yayın başlatılmadı. Native derleme, kaynak ve ses adımları tamamlandıktan sonra tek seferde yapılacak.
