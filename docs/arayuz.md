# Arayüz yönü

## Görsel dil

Çalışan ana arayüz Rust/Slint ile derlenir. WebView kullanılmaz. Mevcut kütüphane, favoriler, Steam kaynakları ve kaynak ölçümü native bileşenlerdir. 21st.dev referansları yalnızca tasarım yönünü belirler; Discord görünümü de aynı native bileşen sistemiyle hazırlanacak.

Oyun kapaklarının öne çıktığı, koyu grafit bir masaüstü arayüzü. Tek vurgu rengi soluk yeşil; Discord bağlantı durumunda gerektiğinde mor. Büyük gradient arka planlar, parlayan çerçeveler, sürekli hareket ve gereksiz istatistik kartları kullanılmayacak.

| Öğe | İlk tasarım kararı |
| --- | --- |
| Ana arka plan | `#101214` |
| Panel / kart yüzeyi | `#181B1F` |
| İnce ayrım çizgisi | `#2A3036` |
| Birincil metin | `#F1F3F5` |
| İkincil metin | `#ABB3BD` |
| Vurgu | `#B6E88B`, koyu buton metni |
| Köşe | Kart 12 px, giriş ve buton 8 px |
| Tipografi | Yerel Segoe UI / sistem fontu; 13–14 px gövde |
| Hareket | 100–160 ms opacity/transform; reduced-motion desteği |

Kontrast, DPI ve klavye erişimi tasarım denemesinde doğrulanacak. Ağdan font indirme gerekmeyecek. Açık tema aynı token sistemiyle sonraki aşamada eklenebilir.

## Pencere düzeni

Varsayılan tasarım referansı 1440 × 900. Sol navigasyon 208 px, sağ sosyal panel 288 px ve gizlenebilir. Merkez alan genişliğe göre 3–6 kapak sütunu gösterir. 1024 px ve altında sosyal panel çekmeceye dönüşür; yüksek DPI'da içerik kesilmez.

```text
┌──────────────────────────────────────────────────────────────────────┐
│ Game Library                  Oyun ara                       ─ □ ×  │
├──────────────┬──────────────────────────────────┬────────────────────┤
│ Kütüphane    │ Kütüphanen                       │ Discord            │
│ Favoriler    │ Tümü · Kurulu · Son oynanan       │ Seçili kişiler     │
│ Koleksiyonlar│                                  │ Kişi / aktivite    │
│              │  Kapak       Kapak       Kapak   │ Kişi / aktivite    │
│ Kaynaklar    │  Oyun adı    Oyun adı    Oyun adı │                    │
│ Steam        │                                  │ İzlenen odalar     │
│ Epic         │  Kapak       Kapak       Kapak   │ Oda / üye sayısı   │
│ Riot         │  Oyun adı    Oyun adı    Oyun adı │ Oda / üye sayısı   │
│ Diğer        │                                  │                    │
│              │                                  │ Yayın kontrolleri  │
│ Ayarlar      │                                  │ Erişime bağlı      │
└──────────────┴──────────────────────────────────┴────────────────────┘
```

Bu bir yerleşim şemasıdır; çalışan ürün veya nihai görsel tasarım değildir.

## Ekranlar

Kütüphane: arama, kaynak filtresi, kurulum durumu, sıralama ve kapak boyutu. Kartın üzerinde yalnızca kapak, başlık ve gerekli kaynak/durum bilgisi. Hover'da başlat düğmesi çıkabilir, fakat klavye ve bağlam menüsüyle aynı eylem sürekli erişilebilir olmalı.

Oyun ayrıntısı: sınırlı hero görseli, belirgin Oyna düğmesi, kurulum seçimi, son oturumlar, koleksiyon ve kapak değişimi. Aynı oyunun iki mağaza kurulumu varsa başlatılacak kurulum seçilir.

Sosyal panel: bütün Discord arayüzünü yeniden kurmak yerine seçili kişiler ve izlenen ses odaları. Durum alınamıyorsa bilinmiyor gösterilir. Kullanıcı adına DM erişimi varsa görüşme çekmecesi; yoksa Discord'da açma eylemi.

Yayın alanı: başlat/durdur, oyun/pencere kaynağı, kalite/çözünürlük, FPS ve ses kontrolleri. Yayın sesi ile mikrofon/sesli görüşme ayarları ayrı tutulur. Yalnızca doğrulanmış yetenekler kullanılabilir olur; sınırlı erişim ilgili ayarın yanında açıklanır. Yayın ve uygulanmış ayarlar Discord'dan alınan gerçek durumu gösterir.

Ayarlar: oyun kaynakları ve tarama kökleri, kapak sağlayıcıları, Discord hesabı, kaynak tasarrufu ve isteğe bağlı istemci eklentileri. Tarama, kapak eşleştirme ve bağlantı hataları ilgili bölümde gösterilir.

## İlk açılış

Launcher yolları bulunur, tarama başlar ve sonuçlar gelir gelmez liste görünür. İnternet ve Discord bağlantısı beklenmez. Kullanıcıya sonradan isteğe bağlı hesap bağlama, kapak sağlayıcısı ve ek oyun klasörü seçenekleri sunulur. Boş kütüphane durumu elle oyun ekleme eylemini içerir.

## 21st.dev araştırması

Üç referans incelendi:

- [Dashboard Sidebar — Arun Dass](https://21st.dev/@arunjdass/components/dashboard-sidebar): dar navigasyon, iki tema ve ince yüzey ayrımları için referans.
- [Dashboard with Collapsible Sidebar — Sonu kumar](https://21st.dev/@uniquesonu/components/dashboard-with-collapsible-sidebar): açılıp kapanan navigasyon ve uygulama kabuğu için referans.
- [Chat template — Manoj](https://21st.dev/@rayimanoj8/components/chat-template/whatsapp-mock): görüşme listesi ve mesaj alanı oranları için referans.

Hazır dashboard'un grafik ve istatistik alanları bu ürüne taşınmayacak. Yerleşim fikirleri oyun kütüphanesine uyarlanacak; alınan kodun lisansı ve bağımlılıkları parça bazında kontrol edilecek. Bu aşamada üçüncü taraf bileşen kodu kopyalanmadı.

## Kabul kriterleri

- 1.000 oyun ve uzun başlıklarda okunabilir grid; aynı düzen 10.000 kayıtta sanallaştırılmış çalışır.
- %100, %150 ve %200 DPI'da başlıklar ve pencere kontrolleri çakışmaz.
- Klavyeyle arama, kart seçme, başlatma, modal kapatma ve kaynak değiştirme.
- İnternet kapalıyken hiçbir tam ekran bekleme veya boş kapak nedeniyle layout kayması olmaz.
- Sosyal panel kapalıyken gereksiz render ve görsel yükleme oluşmaz.
- Görsel önizlemede örnek veriler açıkça belirtilir; gerçek bağlantı varmış gibi gösterilmez.
