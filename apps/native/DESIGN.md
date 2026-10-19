---
name: Game Library — Midnight Arcade Shelf
description: Windows için kapak odaklı, gece mavisi yüzeyler ve lime eylemler kullanan yerel oyun kütüphanesi.
colors:
  canvas: "#101321"
  sidebar: "#141829"
  surface: "#1b2137"
  hover: "#282f48"
  line: "#303953"
  text: "#f2f4fa"
  muted: "#b6bfd4"
  subtle: "#929eb9"
  accent: "#d4f879"
  accent-surface: "#293523"
  accent-surface-hover: "#35452c"
  accent-ink: "#18200f"
  accent-hover: "#e2ff9a"
  accent-pressed: "#b8dd5d"
  secondary: "#beb2f4"
  positive: "#97ddb3"
  warning: "#f1c781"
  danger: "#f2a1ad"
typography:
  headline:
    fontFamily: "Segoe UI"
    fontSize: "28px"
    fontWeight: 700
  headline-short:
    fontFamily: "Segoe UI"
    fontSize: "24px"
    fontWeight: 700
  detail-title:
    fontFamily: "Segoe UI"
    fontSize: "24px"
    fontWeight: 600
  heading:
    fontFamily: "Segoe UI"
    fontSize: "15px"
    fontWeight: 600
  card-title:
    fontFamily: "Segoe UI"
    fontSize: "14px"
    fontWeight: 600
  body:
    fontFamily: "Segoe UI"
    fontSize: "13px"
  action:
    fontFamily: "Segoe UI"
    fontSize: "13px"
    fontWeight: 600
  label:
    fontFamily: "Segoe UI"
    fontSize: "11px"
  eyebrow:
    fontFamily: "Segoe UI"
    fontSize: "10px"
    fontWeight: 600
rounded:
  status: "6px"
  count: "7px"
  control: "8px"
  navigation: "10px"
  panel: "12px"
spacing:
  compact: "4px"
  related: "8px"
  control: "12px"
  inset-top-short: "14px"
  regular: "16px"
  inset-compact: "20px"
  inset-wide: "28px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    typography: "{typography.action}"
    rounded: "{rounded.control}"
    height: "36px"
    padding: "0px 12px"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.control}"
  button-primary-pressed:
    backgroundColor: "{colors.accent-pressed}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.control}"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    height: "36px"
    padding: "0px 12px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    height: "36px"
    padding: "0px 12px"
  navigation-active:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    typography: "{typography.action}"
    rounded: "{rounded.navigation}"
    height: "42px"
    padding: "0px 12px"
  navigation-filter-active:
    backgroundColor: "{colors.accent-surface}"
    textColor: "{colors.accent}"
    rounded: "{rounded.navigation}"
    height: "42px"
    padding: "0px 12px"
  field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    height: "38px"
    padding: "0px 13px"
  tab-active:
    backgroundColor: "{colors.accent-surface}"
    textColor: "{colors.accent}"
    typography: "{typography.action}"
    rounded: "{rounded.control}"
    height: "34px"
    padding: "0px 12px"
  panel:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.panel}"
  game-cover:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.panel}"
  count-badge:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.secondary}"
    typography: "{typography.label}"
    rounded: "{rounded.count}"
    height: "26px"
    padding: "0px 9px"
  friend-row:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.navigation}"
    padding: "10px"
---

# Design System: Game Library

## Overview

**Creative North Star: "Midnight Arcade Shelf"**

Koyu gece mavisi raf, oyun kapaklarını öne çıkarır. Lime eylemler, lavanta yardımcı işaretler ve kısa Türkçe metinler kütüphaneye sıcak, oyun odaklı bir karakter verir. Sık kullanılan denetimler belirgin; kaynak yönetimi ve yardımcı işlemler kendi alanlarında kalır.

Bu belge Windows için Rust/Slint arayüzünü tanımlar. Çalışan sistemin kaynağı [design.slint](ui/design.slint) ve [library.slint](ui/library.slint) dosyalarıdır. Google Stitch [tasarım projesi](https://stitch.withgoogle.com/projects/15089961104543268171), `9dda607a15734f3d81a880f50bc2793f` ekranıyla görsel yönü sağladı. Konseptteki örnek oyunlar, kişiler, sıralama seçenekleri ve başka desteklenmeyen denetimler ürün özelliği sayılmaz. Segoe UI, native pencere, software renderer ve mevcut veri akışı korunur.

[Sidecar](.impeccable/design.json) içindeki HTML/CSS, native bileşenlerin bağımsız dokümantasyon önizlemesidir. Sentezlenen tonal renk şeritleri de bu önizlemeye aittir; çalışma zamanına eklenen renk ölçeği değildir. Üstteki tokenlar ve Slint kaynakları uygulamanın gerçek değerlerini taşır.

**Key Characteristics:**

- Kapak odaklı, uyarlanan oyun rafı.
- Gece mavisi yüzeyler, lime ana eylem ve lavanta yardımcı vurgu.
- Ayrı gezinme, platform filtresi ve arkadaş alanı.
- Gerçek kayda bağlı sayaçlar, durumlar ve eylemler.
- Fare ve klavyede aynı işlere erişim.

## Colors

Renk değerleri üstteki token tablosundan alınır; `Theme` adlarıyla eşleşir.

### Primary

`accent` lime ana eylem, etkin sayfa ve odak rengidir. `accent-ink` bu açık yüzey üzerindeki koyu yazıdır. `accent-hover` ve `accent-pressed` etkileşimi gösterir; `accent-surface` ve `accent-surface-hover` seçili filtre ve sekmelere daha sakin bir yüzey verir.

### Secondary

`secondary` lavanta oyun sayısı, kapaksız oyun simgesi, boş arkadaş alanı ve ses kanalı bilgisinde kullanılır.

### Neutral

`canvas` ana zemin, `sidebar` iki yan sütun, `surface` panel ve alan, `hover` üzerine gelinen ikincil denetim, `line` ayırıcı ve basılı yüzeydir. `text`, `muted` ve `subtle` ana bilgi, açıklama ve yardımcı bilgiyi ayırır.

`positive` çevrimiçi/kurulu, `warning` boşta, `danger` rahatsız etmeyin ve tehlikeli eylem rengidir. Durumlar metinle birlikte sunulur; çevrimdışı veya bilinmeyen durum `subtle` kullanır.

**The State Has Meaning Rule.** Durum rengi yalnız verideki durumu anlatır; bağlı olmayan Discord oturumu güncel çevrimiçi durum gibi gösterilmez.

## Typography

Tüm arayüz Windows'un yerel **Segoe UI** ailesini kullanır. Yeni font dosyası veya ağ fontu eklenmez. Slint'in varsayılan satır ölçüsü korunur; kodda olmayan satır yüksekliği veya harf aralığı tokenı tanımlanmaz.

Sayfa başlığı `headline`, 720 px altındaki kısa pencerede `headline-short` rolündedir. Oyun ayrıntısının başlığı `detail-title`, panel başlığı `heading`, kapak altı oyun adı `card-title` rolündedir. Gövde ve alanlar `body`; kaynak/boyut ve durum açıklamaları `label`; küçük bölüm işaretleri `eyebrow` kullanır. Ana düğmeler orta kalın, kart üstündeki Oyna yazısı 700 ağırlığındadır. Uzun oyun/arkadaş adları satır düzenini genişletmeden kesilir; uygun açıklamalar satıra sarılır.

## Layout

Üç bölüm: solda gezinme, ortada esneyen içerik, sağda isteğe bağlı arkadaş paneli. Pencere genişliği 1100 mantıksal px altında gezinme 192 px, arkadaş paneli 256 px ve yatay içerik boşluğu 20 px olur; diğer genişliklerde sırasıyla 208, 280 ve 28 px kullanılır. Arkadaş paneli kapatıldığında boşalan alan oyun rafına katılır.

Üst arama bölümü 70 px yüksekliğindedir. Pencere yüksekliği 720 mantıksal px altındaysa bu bölüm 62 px, sayfa başlığı 24 px, içerik üst boşluğu 14 px ve ana içerik aralığı 12 px olur; normal değerler sırasıyla 70, 28, 20 ve 16 px'dir. Kısa pencerede kütüphane başlığı altındaki yardımcı cümle gizlenerek raf alanı korunur.

Platformlar sol menüde Tüm platformlar, Steam, Epic Games, Riot Games ve Elle eklenen olarak listelenir. Kütüphane üzerindeki Tümü/Kurulu sekmeleri kurulum filtresidir. Arama, seçili platform ve koleksiyon mevcut veri modelini süzer. Gezinme simgeleri ve sayaçlar satır ortasına oturur; yan panel ayırıcıları panel sınırlarında kalır.

Raf sütunu kullanılabilir genişlikten hesaplanır: `max(2, min(12, floor((shelf-width + 16px) / 206px)))`. Sütun aralığı 16 px, kapak oranı 2:3, kartın başlık/meta alanı 66 px'dir. İşçi tarafındaki sütun sınırı da 2–12'dir. Sayfa 100 kayıtla sınırlı, satırlar sanallaştırılmıştır; yeniden boyutlandırma yeni bir veri veya sıralama özelliği oluşturmaz.

1280 px altındaki ayarlar sekmeleri bölüm seçicisine dönüşür; açık sohbet kişi listesini dar yerleşimde geri çeker. Aynı dar yerleşimde açık görüşme varsa genel Discord sayfa başlığı gizlenir; görüşme adı korunur ve mesaj alanına yer kalır. Sohbet 900 px yükseklik altında sıkışır, 850 px altında açılan sohbet yayın panelini kapatır. Bu eşikler yerel pencerenin mantıksal ölçüleridir; web veya mobil kırılımları değildir.

## Elevation & Depth

Derinlik yüzey tonu, ince ayırıcı, kapak resmi ve odak kenarlığıyla kurulur. Ortak bileşenlerde gölge, blur, parallax veya sürekli dekoratif animasyon bulunmaz. Renk ve görünürlük durumları ek zamanlayıcı gerektirmeden değişir.

**The Quiet Surface Rule.** Durağan yüzeyler sakin kalır; vurgu etkileşimi, seçimi ve veriye bağlı durumu anlatır.

## Shapes

Kontroller yumuşak köşeli, gezinme satırları biraz daha yuvarlak, kapak ve paneller daha geniş köşelidir. `status`, `count`, `control`, `navigation`, `panel` köşe tokenları bu sırayı tanımlar. Arkadaş avatarı 36 px kare içinde 12 px köşe; presence noktası 12 px çapındadır. Kapakların software renderer'da görünür köşeleri yerel SVG maskesiyle tamamlanır; kaynak kapak dosyası değiştirilmez.

## Components

### Buttons

`Action` ana, ikincil, şeffaf, seçili ve tehlikeli durumları taşır. Varsayılan yüksekliği 36 px; yatay iç boşluğu 12 px, ikon/yazı arası 8 px'dir. Odakta 2 px kenarlık, devre dışıyken 0,6 opaklık kullanılır. Enter ve Boşluk erişilebilir varsayılan eylemi çalıştırır. Kart üstü Oyna alanı 38 px yüksekliğinde, favori alanı 36 px karedir.

### Navigation and tabs

`Navigation` 42 px yüksekliğinde ve 12 px yatay boşlukludur. Etkin ana sayfa dolu lime, etkin platform filtresi koyu lime yüzeydedir. Sayaçlar gerçek kayıttan gelir. `Tab` 34 px yüksekliğindedir; seçili sekme koyu lime yüzey ve lime yazıyla ayrılır. Alt menüde Discord ve Ayarlar bulunur.

### Cards

Kapak, ad ve kaynak/boyut bilgisi tek oyun kaydını temsil eder. Kaynak yanında kurulum noktası vardır. Fare kapağın üzerindeyken veya kart/eylem odaktayken lime çerçeve ve kurulu oyunda Oyna alanı görünür. Seçili favori yıldızı sürekli görünür. Kurulu olmayan oyunun durum etiketi gösterilir; başlatma etkin değildir.

Kapağa veya ada tıklamak ayrıntıyı açar. Kartta Enter/Boşluk ayrıntı, P başlatma, F favori eylemidir. Sağ tık menüsü aynı eylemleri sağlar. Eksik kapak, başlık ve kaynakla yerel bir yer tutucu gösterir; örnek kapak indirilmez.

### Inputs

`Field` 38 px yüksekliğinde, 1 px çizgili ve 13 px yatay boşlukludur. Odak çizgisi lime olur; seçili yazı lime zemin üzerinde koyu mürekkeptir. Üst alan oyun arar; Discord sayfasında arkadaş arama alanı gösterilir. Ctrl+F kütüphane aramasına erişir.

### Panels and count badges

`Panel` çerçevesiz yükseltilmiş yüzeydir; iç boşluk tüketen ekrana aittir. Sayfa başlığındaki 26 px yükseklikteki lavanta yazılı sayaç eşleşen gerçek oyun sayısını gösterir. Önceki/Sonraki yalnız birden fazla sayfada görünür; alt kayıt aralığı her zaman filtre sonucunu açıklar.

### Friends

Arkadaş satırı durum, oyun ve ses bilgisine göre 64, 80, 96 veya 112 px yüksekliğe sahiptir. Çevrimiçi yeşil, boşta amber, rahatsız etmeyin pembe, çevrimdışı/bilinmeyen nötrdür. Okunmamış mesaj sayısı, presence ve favori ayrı işaretlerdir. Bağlantı kesildiğinde sağ panel son bilinen durum olduğunu söyler; güncel oyun/ses verisi gibi davranmaz.

Satır görüşmeyi açar; favori eylemi ayrıca adlandırılmış denetim ve F kısayoludur. Boş arkadaş panelindeki açıklama yalnız içeriğinin gerektirdiği yüksekliği kaplar. Örnek kişiler yalnız geliştirme fixture'larında ve görünür “Örnek kişiler · görsel doğrulama” etiketiyle gösterilir. Gerçek kullanıcının arkadaşları veya mesajları konsept içeriğiyle değiştirilmez.

## Do's and Don'ts

### Do:

- Do yeni ekranlarda mevcut `Theme`, `Action`, `Field`, `Navigation`, `Tab` ve `Panel` bileşenlerini kullan.
- Do seçim, odak ve kurulum/bağlantı durumunu metin ve erişilebilir eylemlerle birlikte göster.
- Do uzun adları, dar pencereyi, boş sonuçları ve arkadaş paneli açık/kapalı düzenini doğrula.
- Do kapak çözme, 100 kayıtlık sayfa ve sanallaştırılmış satır sınırlarını koru.
- Do doğrulama fixture'larını görünür şekilde etiketle ve gerçek kullanıcı verisinden ayır.

### Don't:

- Don't Stitch konseptindeki uydurma oyun, kişi, sayı veya desteklenmeyen eylemleri gerçek ürün verisi olarak ekle.
- Don't bu native arayüze WebView, ağ fontu, yeni runtime veya sürekli dekoratif zamanlayıcı ekle.
- Don't fareyle görünür olan kart eylemlerini klavye erişiminden çıkar.
- Don't önceki derlemelerin RAM/CPU sonuçlarını bu tasarımın ölçümü olarak sun.
