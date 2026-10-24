# FigCraft ve Pergel: ayrıntılı karşılaştırma

Bu belge, FigCraft açık kaynak tasarım aracını (`storytold/figcraft`, saf Rust + egui) Pergel ile
karşılaştırır. Kapsam:

- Arayüz ve ikonlar
- MCP
- Canvas çekirdeği
- Tasarım sistemi
- Prototipleme ve Dev Mode
- İşbirliği
- Dosya formatları
- Eklentiler
- Paketleme ve test

Amaç, FigCraft'ın bizden iyi olduğu ya da bizde olmayan özellikleri, Pergel'e nasıl ekleneceğini ve
ne kadar zahmetli olduğunu çıkarmaktır.

**Yöntem.** İki kod tabanı yalnızca okundu. FigCraft derlenmedi, çalıştırılmadı. FigCraft'ın
performans ve "parity" yüzdeleri kendi belgelerinden alındı, doğrulanmadı. Pergel'deki hata
bulgularının en önemlileri (bölüm 2) koda tek tek bakılarak teyit edildi; teyit edilenler ✔ ile
işaretli.

**Kısaltmalar**

- **FC** = `C:\Users\burak\Downloads\Pulse DM\Arşiv\figcraft-main\figcraft-main`
- **PG** = bu depo
- Zorluk:
  - **S**: birkaç saat ile 1–2 gün
  - **M**: 3–10 gün
  - **L**: 2 haftadan fazla ya da mimari değişiklik

---

## İçindekiler

1. [Yönetici özeti](#1-yönetici-özeti)
2. [Önce düzeltilmesi gereken Pergel bulguları](#2-önce-düzeltilmesi-gereken-pergel-bulguları)
3. [Mimari](#3-mimari)
4. [MCP, ajan entegrasyonu, CLI, kontrol kanalı](#4-mcp-ajan-entegrasyonu-cli-kontrol-kanalı)
5. [Arayüz, ikonlar, tema, dil, erişilebilirlik, kısayollar](#5-arayüz-ikonlar-tema-dil-erişilebilirlik-kısayollar)
6. [Canvas çekirdeği: vektör, boolean, şekil, stroke, metin, render, efekt, boya, snap](#6-canvas-çekirdeği)
7. [Tasarım sistemi: auto layout, component, kütüphane, stil, değişken](#7-tasarım-sistemi)
8. [Prototipleme](#8-prototipleme)
9. [Dev Mode ve kod üretimi](#9-dev-mode-ve-kod-üretimi)
10. [Sayfa, section, board, widget, datafill, şablon, yorum, sürüm, branch](#10-sayfa-section-board-widget-datafill-şablon-yorum-sürüm-branch)
11. [İşbirliği (canlı oturum)](#11-işbirliği)
12. [Dosya giriş/çıkış](#12-dosya-girişçıkış)
13. [Eklentiler](#13-eklentiler)
14. [Platform, paketleme, CI, test, performans](#14-platform-paketleme-ci-test-performans)
15. [Pergel'in güçlü olduğu yerler (korunacaklar)](#15-pergelin-güçlü-olduğu-yerler)
16. [Lisans: neyi alabiliriz](#16-lisans-neyi-alabiliriz)
17. [Yol haritası ve öncelikli iş listesi](#17-yol-haritası)

---

## 1. Yönetici özeti

| | Pergel | FigCraft |
|---|---|---|
| Yığın | Rust core, Skia (WASM, WebGL2, GPU), Tauri 2, Svelte 5 | Saf Rust, egui + wgpu arayüz, tiny-skia (CPU) render |
| Kod boyutu | yaklaşık 38 bin satır (21,5k Rust + 16,8k TS/Svelte) | yaklaşık 253 bin satır Rust (yaklaşık 90 bini test) |
| Test | 66 `#[test]` | 3.184 `#[test]`, 441 test dosyası, proptest, fuzz, collab simülasyonu |
| Lisans | MIT | MIT OR Apache-2.0 |
| Durum | 0.1, yalnızca Windows'ta test edildi | Erken alfa; Win, mac, Linux, FreeBSD ve Web paketleri var |

**FigCraft'ın açıkça önde olduğu alanlar:**

- **Canvas çekirdeği:**
  - Gerçek vektör ağları
  - Canlı boolean grupları ve shape builder
  - Değişken genişlikli stroke
  - Squircle, yay ve halka şekiller
  - Progressive blur, glass ve noise efektleri
  - Alpha/luminance maske
  - Tam affine dönüşüm
- **Tasarım sistemi derinliği:**
  - Min/max boyut
  - Grid iz tipleri ve span
  - Slot'lar
  - Frame başına değişken modu
  - Scope'lar
  - Değişken import/export
  - Stil ve değişken yayımlama
  - Library swap
- **Prototipleme:**
  - Çoklu eylem
  - İfadeler ve else-if
  - 10 tetikleyici
  - Easing ve spring
  - Gerçek smart animate
  - Cihaz çerçeveleri
- **Dev Mode:**
  - Inspect yüzeyi
  - Kategorili anotasyonlar
  - Kalıcı ölçümler
  - Compare changes
  - 6 dilde kod üretimi
- **İşbirliği:**
  - Sunucu sıralı, özellik bazlı birleştirme
  - Kişi başı undo
  - Offline kuyruk
  - TLS ve izinler
  - Cursor chat ve spotlight
  - Self-hosted sunucu
- **Arayüz:**
  - Açık, koyu ve sistem teması
  - 19 dil (**Türkçe dahil**)
  - Arayüz boyutu ayarı
  - Erişilebilirlik (AccessKit, F6, odak halkası)
  - Tek kaynaklı kısayol haritası ve kısayol paneli
  - 245 öğelik menü
  - Tercihler penceresi
- **Dosya ve platform:**
  - `.fig` içe aktarma (best effort)
  - PDF baskı seçenekleri (bleed, crop marks, CMYK)
  - sRGB/P3 ve ICC
  - İmzalı paketler
  - Kapsamlı CI
- **MCP güvenilirliği:**
  - Hata anında rollback
  - `undo` ve `redo` araçları
  - `batch` + `$step`
  - Yanıt byte bütçesi
  - Dosya erişim kökleri
  - Headless mod ve CLI
  - Fuzz testleri

**Pergel'in önde olduğu alanlar** (ayrıntı bölüm 15):

- **Tasarımdan koda MCP:**
  - `get_design_context`
  - Tekilleştirilmiş varlık dosyaları
  - `compare_to_design` piksel farkı
  - Kod tabanı tarayan Code Connect önerileri
  - React + Tailwind çıktısı
  - Iconify ve Mermaid
- **Figma'dan link ile canlı içe aktarma** ve **Figma JS eklenti uyumluluğu** (FigCraft'ta ikisi de yok).
- **Render ve indeks:**
  - GPU render: 100 bin katmanda yaklaşık 22 ms. FigCraft CPU'da ve GPU'yu kendi açık listesinde tutuyor.
  - R-tree indeks: 100 bin katmanda hit-test 1 µs'nin altında.
- **Web yığınının bedavaya getirdikleri:**
  - IME, bidi ve CJK
  - Yazım denetimi
  - Katman küçük resimleri
  - ContextBar
  - Ana ekranda Drafts, Starred ve Trash
- **Geliştirme hızı:** küçük ve kavranabilir kod tabanı.

**Kısa sonuç.** FigCraft "Figma'nın tamamını taklit et" yolunda çok daha ileride. Pergel ise
"ajanların tasarımı doğru koda çevirmesi" konusunda daha keskin. En hızlı getiri sağlayacak işler
şöyle sıralanabilir:

1. Bölüm 2'deki güvenlik ve doğruluk hataları
2. Tek kısayol haritası, tema ve i18n (FigCraft'ın `tr.toml`'u yeniden kullanılabilir)
3. Taffy'nin bedava verdiği auto layout eksikleri
4. Skia'nın bedava verdiği render ve metin eksikleri
5. MCP'ye rollback, undo ve batch

---

## 2. Önce düzeltilmesi gereken Pergel bulguları

FigCraft'ı incelerken bizim kodda görülen hatalar ve açıklar aşağıda. ✔ işaretliler koda bakılarak
teyit edildi.

**Durum (2026-10-25).** Bu bölümdeki hataların hepsi düzeltildi:

- **G1:** Figma eklentileri artık ayrı bir JavaScript motorunda (QuickJS) çalışıyor; uygulamaya, diske ve ağa erişimleri yok. Diske yalnızca kullanıcının dosya diyaloğunda seçtiği yerlere okuma/yazma yapılabiliyor (`pick_path`). Dosya id'leri doğrulanıyor; bu, bulunan bir path traversal açığını da kapattı.
- **G4:** Bir ajan yeni bir klasöre ilk kez erişmek istediğinde Pergel izin soruyor.
- **D2, D8:** Canlı oturumda yalnızca değişen parçalar gönderiliyor. Undo, kişinin yalnızca kendi değişikliklerini geri alıyor.
- **Diğerleri:** G2, G3, D1, D3, D4, D5, D6, D7, D9, D10, D12 düzeltildi; PR CI'ı eklendi.

### 2.1 Güvenlik

| # | Bulgu | Yer | Öneri | Zorluk |
|---|---|---|---|---|
| G1 ✔ | **Figma eklentileri sandbox'sız çalışıyor.** Ana betik sayfanın içinde `new Function(...)` ile çalışıyor; `tauri.conf.json`'da `"csp": null`. Kötü niyetli bir eklenti Tauri komutlarını (ör. keyfi yola yazan `write_file`, `file_delete`) çağırabilir. | `src/lib/figmaPlugin.svelte.ts:548`, `src-tauri/tauri.conf.json:26`, `src-tauri/src/files.rs:284` | Kısa vadede Tauri capability'leriyle komutları kısıtla, CSP koy, `write_file`'ı yalnızca diyalogdan dönen yollarla sınırla (S). Asıl çözüm: eklentiyi QuickJS-wasm içinde çalıştırmak. Figma'nın kendi yaklaşımı bu ve senkron API'yi koruyor (M/L). | S → M/L |
| G2 ✔ | **Canlı oturum token'ı kriptografik değil.** Zaman ve pid ile beslenen xorshift kullanılıyor. Sunucu `0.0.0.0:3856`'yı dinliyor, bağlantı düz `ws://`, token URL'de taşınıyor, karşılaştırma sabit zamanlı değil, mesaj başına 64 MB'a izin var, hız sınırı yok. | `src-tauri/src/live.rs:60`, `:128` | 128 bit OS RNG (`getrandom`), sabit zamanlı karşılaştırma, yalnızca LAN IP'sine bind, token'ı ilk `Hello` mesajında gönderme, limitler ve throttle (FC `crates/collab-net/src/{token,limits,throttle}.rs`) | S |
| G3 ✔ | **`/assets` rotası `Access-Control-Allow-Origin: *` ile servis ediliyor** ve rmcp'nin Host kontrolünün dışında. Dosya adını tahmin eden herhangi bir web sayfası varlıkları okuyabilir. | `src-tauri/src/mcp.rs:1009`, `:1023` | CORS'u kaldır ya da rotayı aynı Host allowlist'ine al | S |
| G4 | **MCP dosya erişimi sınırsız.** `create_nodes` içindeki `src`/`path` alanları ve `compare_to_design` herhangi bir dosyayı okuyabiliyor; `download_assets` ve `get_design_context` herhangi bir klasöre yazabiliyor. | `mcp.rs:340-368`, `:305-319`, `:770` | Tek bir `file_access` modülü, izinli klasör listesi ve canonicalize (FC `crates/engine/src/file_access.rs`) | M |

### 2.2 Doğruluk

| # | Bulgu | Yer | Öneri | Zorluk |
|---|---|---|---|---|
| D1 ✔ | **Canlı oturumda ıraksama.** Aynı üst-düzey frame iki tarafta değişirse her taraf karşı tarafın sürümünü alıyor (testin adı bile `the_same_frame_edited_on_both_sides_takes_the_other_peer`). Kimse yeniden göndermediği için iki kopya kalıcı olarak farklı kalıyor. | `crates/pergel-core/src/merge.rs:255`, `src/lib/live.svelte.ts` `apply()` | Deterministik eşitlik bozucu (peer id ya da Lamport sayacı). Orta vadede sunucu sıralaması (bölüm 11). | S |
| D2 | **Canlı oturumda undo çalışmıyor.** Uzaktan doküman geldiğinde `pergel_doc_replace` → `Scene::from_trees` → `Scene::default()` çağrılıyor ve geçmiş sıfırlanıyor. | `crates/pergel-wasm/src/main.rs:1327`, `document.rs:79` | Uzak değişikliği geçmişi koruyarak uygula (node bazlı yama) | M |
| D3 | **Yeniden bağlanınca offline yapılan işler kayboluyor.** Bağlantı koparken `base=""` oluyor; tekrar katılınca uzak doküman olduğu gibi alınıyor ve autosave bunu diske yazıyor. | `live.svelte.ts` | `base`'i koru, yeniden bağlanınca üç yönlü birleştir | S–M |
| D4 ✔ | **MCP/agent adımları hata olsa da commit ediliyor.** `step()` sonuç ne olursa olsun `commit` çağırıyor; `run_script` de hata ya da zaman aşımında commit ediyor. Kısmi değişiklikler dokümanda kalıyor. | `crates/pergel-wasm/src/main.rs:1122-1132`, `src/lib/plugins.ts` `finish` | `begin` öncesi snapshot al, hata olursa geri yükle (`abort()`) | M |
| D5 ✔ | **Diamond gradient radial olarak çiziliyor.** `gradient_shader` yalnızca Linear, Angular ve "geri kalan her şey radial" dallarını biliyor. | `crates/pergel-render/src/lib.rs:676-689` | SkSL runtime shader ile `t = abs(u) + abs(v)` | S |
| D6 ✔ | **Flip içeriği aynalamıyor.** Yalnızca konum ve rotation aynalanıyor; vektör yolu, metin ve görsel aynalanmıyor (yorumda "Shapes are symmetric" varsayımı var). | `crates/pergel-core/src/scene.rs:997-1015` | `flip_x/flip_y` bayrağı ve render'da `canvas.scale(-1,1)`, vektör yolunu aynalama (S–M). Kalıcı çözüm affine (bölüm 6.10). | S–M |
| D7 ✔ | **Release'te `panic = "abort"`.** Yaklaşık 166 `unwrap`/`expect` var; WASM'da tek bir panik tüm tuvali düşürüyor. | `Cargo.toml:28` | Panic hook ve guard; clippy `unwrap_used` kuralı (FC `crates/engine/src/guard.rs`) | S–M |
| D8 | **Her canlı değişiklikte tüm doküman JSON'u yayınlanıyor**, base64 görseller dahil. | `live.svelte.ts` `tick()` | Node bazlı delta. `History` zaten dokunulan node'ları tutuyor. | M |
| D9 | **MCP zaman aşımı mesajı yanıltıcı.** Webview isteği zaman aşımından sonra da uygulayabilir, ama mesaj "işlem uygulanmış olabilir, önce kontrol et" demiyor. | `mcp.rs:68-73` | Mesajı düzelt; istek id'sine göre JS tarafında iptal | S |
| D10 | **`delete_nodes` instance içindeki id'leri sessizce atlıyor**, uyarı dönmüyor. | `main.rs:6001-6008` | `warnings` alanı ekle | S |
| D11 | **Kısayollar dört yerde ayrı yazılmış ve tutarsız.** Outline stroke bir yerde Ctrl+Shift+O, başka yerde Ctrl+Alt+O; Flatten bir yerde Ctrl+E, başka yerde Alt+Shift+F. macOS'ta ⌘ gösterimi yok. | `CanvasView.svelte:379-516`, `commands.ts`, `layerMenu.ts`, `Toolbar.svelte` | Tek keymap (bölüm 5.6) | M |
| D12 | **Eşzamanlı MCP istekleri serileştirilmiyor.** `run_script`'in açtığı adıma başka aracın değişiklikleri karışabilir (çıkarım, doğrulanmadı). | `src/lib/mcp.ts` `handle` | Async mutex ya da kuyruk | S |

---

## 3. Mimari

### 3.1 Pergel

- **Crate'ler:**
  - `crates/pergel-core` (grafiksiz): scene, layout (taffy), components, codegen, SVG (usvg), Figma REST dönüştürücü, merge, history.
  - `crates/pergel-render`: Skia; LOD mesh ve PDF.
  - `crates/pergel-wasm`: C ABI ve agent araçları; `Engine` yaklaşık 6.500 satır.
  - `src-tauri`: dosyalar, fontlar, live (axum WS relay), MCP (rmcp, HTTP `127.0.0.1:3855`), code_scan.
- **Doküman modeli:** sayfa başına `Scene{nodes: Vec<Node>}` arena, rstar R-tree, global `generation` sayacı. Undo dokunulan node'ların önce/sonra kopyalarını tutuyor, sınır 200 adım.
- **Dosya:** `.pergel` tek bir JSON dosyası; görseller base64 olarak içinde. Autosave 1,5 saniyede bir. Otomatik sürüm 10 dakikada bir alınıyor, en fazla 60 tane; her sürüm görseller dahil tam kopya.

### 3.2 FigCraft

- **Katmanlar:** `cargo xtask layers` bağımlılık yönünü zorunlu kılıyor.

  | Katman | Crate'ler |
  |---|---|
  | L0 | `geom`, `color` |
  | L1 | `doc` (`imbl::OrdMap<NodeId, Arc<Node>>`, kalıcı ve yapısal paylaşımlı) |
  | L2 | `layout` (kendi motoru, taffy yok), `text` (harfrust, skrifa, unicode-bidi), `collab`, `plugin` (wasmi), `pathops` (kurbo, linesweeper) |
  | L3 | `render` (tiny-skia, bantlı paralel, retained), `format` (`.figcraft`, `.fig`, REST) |
  | L4 | `svg`, `pdf` (krilla) |
  | L5 | `engine` (500'den fazla komut, `cmd/*`) |
  | L6 | `ui-egui`, `mcp`, `collab-net` |

- **Uygulamalar:** `apps/figcraft`, `figcraft-cli`, `figcraft-web`, `figcraft-server`.
- **Ana ilke:** her davranış bir komut. Menü, kısayol, CLI, JSON kontrol kanalı, MCP ve eklentiler aynı komut kimliklerini çağırıyor (`crates/engine/src/cmd/mod.rs` `spec!` makrosu, `catalog.rs`).
- **Undo:** belgenin `Arc` snapshot'ı alınıyor; ucuz, sınırsız, her jest bir adım.
- **Değişiklik tespiti:** `Document::changed_ids()` ile `OrdMap::diff`. Retained render ve collab bunu kullanıyor.
- **Dosya:** `.figcraft` bir zip. İçinde `document.json`, `images/<hash>`, sürümler, kütüphane ve `thumbnail.png` var. Önce staging dosyasına yazıp sonra rename ediyor. Kurtarma kopyaları ayrı tutuluyor.

### 3.3 Mimari dersler

1. **Komut registry'si.** FigCraft'ta her yeni özellik kendiliğinden menüye, kısayola, MCP'ye ve
   CLI'ya açılıyor. Pergel'de 34 MCP aracı, yaklaşık 95 Quick Actions komutu, menüler ve
   kısayollar ayrı ayrı yazılmış. Ortak bir `{id, label, menuPath, keys, run, params}` registry'si
   (TS tarafında, wasm `Engine` aksiyonlarına bağlı) bölüm 4, 5.6 ve 5.7'deki işlerin hepsini
   besler. **L**, ama aşamalı yapılabilir.
2. **Node bazlı sürüm ve dirty kümesi.** Her node'a `version: u32`, `Scene`'e `dirty` kümesi
   eklenmesi. Retained render, texture önbelleği, collab delta'sı ve devcompare snapshot'larının
   anahtarı bu olur. `touch()` altyapısı hazır olduğu için **S**.
3. **Zip dosya formatı.** Görselleri JSON'dan ayırmak base64'ün %33 şişkinliğini ve "her sürüm
   tam kopya" sorununu çözer. **M**.

---

## 4. MCP, ajan entegrasyonu, CLI, kontrol kanalı

### 4.1 Felsefe farkı

**FigCraft:** stdio üzerinde **18 genel araç**, yaklaşık 590 motor komutuna açılıyor.

| Grup | Araçlar |
|---|---|
| Komut | `list_commands`, `run_command`, `batch` |
| Okuma | `inspect_document`, `get_node`, `inspect_ui` |
| Düzenleme | `create_node`, `set_properties`, `select` |
| Dosya | `export`, `open_file`, `save_file` |
| Görüntü | `screenshot` |
| Geçmiş | `undo`, `redo` |
| UI sürme | `pointer`, `key`, `type_text` |

- **Resource'lar:** `figcraft://document`, `figcraft://commands`, `figcraft://node/{id}`, `figcraft://command/{id}`
- **Prompt'lar:** `mobile_screen`, `design_system`, `landing_page`, `prototype_flow`. Hepsinin sonunda aynı "VERIFY" eki var.

**Pergel:** Streamable HTTP üzerinde **34 görev odaklı araç**, isimleri Figma Dev Mode MCP ile uyumlu.

| Grup | Araçlar |
|---|---|
| Tasarım bağlamı | `get_metadata`, `get_screenshot`, `get_design_context`, `get_variable_defs` |
| Varlık ve doğrulama | `download_assets`, `compare_to_design`, `search_icons` |
| Figma ve diyagram | `import_figma`, `generate_diagram` |
| Kütüphane | `get_libraries`, `use_library_component`, `publish_library` |
| Yorum | `get_comments`, `add_comment`, `resolve_comment` |
| Betik ve dosya | `run_script`, `create_new_file` |
| Code Connect | `get_code_connect_suggestions`, `add_code_connect_map`, `get_code_connect_map` |
| Tasarım sistemi | `search_design_system`, `set_variables`, `bind_variable` |
| Sayfa, seçim, prototip | `get_pages`, `switch_page`, `get_selection`, `set_interactions` |
| Component | `set_variant`, `get_components`, `create_instance` |
| Düzenleme | `create_nodes`, `update_node`, `delete_nodes`, `select_nodes` |

- **Prompt'lar:** `implement_design`, `design_screen`
- **Resource:** yok.

### 4.2 Karşılaştırma

| Konu | Pergel | FigCraft (dosya) |
|---|---|---|
| Transport | HTTP, `127.0.0.1:3855`, hep açık | stdio; çalışan uygulamaya `--attach PORT` ile bağlanma (`crates/mcp/src/{server,backend}.rs`) |
| Pencere olmadan (headless) | Yok, webview gerekiyor | Var: in-process `Session` ve CPU render (`crates/mcp/src/headless.rs`) |
| Hata anında rollback | **Yok** (D4) | Var: doküman, seçim, sayfa, pano ve undo yığını geri yükleniyor (`crates/engine/src/lib.rs:943-1006`, `:1283+`) |
| `undo`/`redo` aracı | Yok (motorda `pergel_undo` var, `main.rs:2698`) | Var |
| `batch` + `$step` referansı | Yok (iç içe `children` var) | Var, sıkı doğrulamalı, iç içe derinlik sınırı 64 (`lib.rs:124-160`) |
| Yanıt byte bütçesi | Yok (yalnızca 3000 katman sınırı) | Varsayılan 8 MiB, `maxBytes`, derinlik için ikili arama, `truncated`, `offset`/`nextOffset` (`tools.rs:581-861`) |
| Screenshot | 64–2048, varsayılan 1024 | 16–8192, varsayılan 1600; gölge ve taşma dahil sınıra uyana kadar yeniden render (`tools.rs:928-968`) |
| Export / save aracı | Yok | png, jpg, svg, webp, pdf (bleed, crop, CMYK), fig; inline ya da diske |
| Dosya erişim kökleri | Yok (G4) | `--automation-read-root` / `--automation-write-root`, clippy ile zorunlu (`file_access.rs`) |
| Panic izolasyonu | Yok | `catch_unwind`, `guard.rs` |
| Test | Yok | 1.260 satır entegrasyon testi: protokol, fuzz (`garbage.rs`), attached, roots, response limit, pixel limit |
| Çoklu sayfa / pencere | Yalnızca aktif sayfa | `allPages`, her metotta `window` parametresi |
| UI sürme | Yok | `pointer`, `key`, `type_text`, `inspect_ui` |
| `initialize` talimatları | `GUIDE`, `NODE_SPEC` | Ayrıca backend modu ve izinli kökler bildiriliyor (`server.rs:31-42`, `:193-206`) |

### 4.3 CLI (FigCraft `apps/figcraft-cli`)

| Komut | Ne yapar |
|---|---|
| `info` | Dosya özeti; eksik fontlar dahil |
| `export` | png, jpg, svg, webp, pdf, fig; `--scale`, `--page`, `--node`; PDF için `--bleed`, `--crop-marks`, `--cmyk` |
| `convert` | Formatlar arası dönüştürme |
| `render` | En fazla 16.384 px |
| `run script.jsonl` | Ya hep ya hiç, `$step` destekli |
| `commands` | Komut kataloğu |
| `mcp` | MCP sunucusu |
| `plugin publish` | Eklenti yayımlama |
| `perf` | Sentetik dokümanlarla benchmark |

`--no-system-fonts` ile deterministik render alınabiliyor. Exit kodları 0, 1, 2.

**Pergel'de CLI yok.** Bunun önündeki engel, agent mantığının wasm'daki `Engine` içinde ve kısmen
TypeScript'te olması.

### 4.4 JSON kontrol kanalı (FigCraft `apps/figcraft/src/control_server.rs`, `docs/control-protocol.md`)

- **Başlatma:** `figcraft --control 7979`; satır başına bir JSON.
- **Metotlar:**
  - `engine.execute`
  - `document.inspect`
  - `node.get`
  - `ui.inspect`: paneller, frame süreleri, yeniden çizim nedenleri
  - `ui.pointer`, `ui.key`, `ui.type`, `ui.input`
  - `ui.menu.invoke`
  - `ui.set`: 40'tan fazla UI durumu
  - `ui.screenshot`, `ui.render`
  - `window.*`, `app.quit`
- **Sınırlar:** en fazla 8 bağlantı, en fazla 8 istekten oluşan kuyruk, 60 saniye zaman aşımı. JSON olmayan ilk satır bağlantıyı kapatıyor; bu, tarayıcıdan gelen saldırılara karşı koruma.
- **Pergel'de karşılığı yok.** Yalnızca dev build'de `window.__pergel` var.

### 4.5 Pergel'e eklenecekler

| # | Özellik | FigCraft referansı | Zorluk |
|---|---|---|---|
| M1 | `undo` / `redo` araçları | `tools.rs:273,435` | S |
| M2 | Hata anında rollback (D4) | `engine/src/lib.rs:973-1006` | M |
| M3 | Gizli araçları MCP'ye açmak: `move_node`, `clone_node`, `get_node_details`, `list_children`, `layers_at` | – | S |
| M4 | `export` ve `save_file` araçları (`exporter.ts` hazır) | `tools.rs:874-989` | S–M |
| M5 | MCP resource'ları: `pergel://document`, `pergel://node/{id}`, `pergel://tools` | `resources.rs` | S |
| M6 | Yeni prompt'lar ve ortak VERIFY eki | `prompts.rs` | S |
| M7 | `batch` + `$step` | `lib.rs:124-160` | S–M |
| M8 | Yanıt byte bütçesi; `get_metadata` için `depth` ve `offset` | `tools.rs:581-861` | S–M |
| M9 | Zaman aşımı mesajı ve istek kuyruğu (D9, D12) | `backend.rs:150-174` | S |
| M10 | Dosya erişim kökleri (G4) | `file_access.rs` | M |
| M11 | MCP testleri: Engine::agent birim testleri, protokol testleri, rastgele JSON ile fuzz | `crates/mcp/tests/it/*` | M |
| M12 | stdio→HTTP proxy binary'si (`pergel-mcp-stdio`): Claude Desktop config uyumu | – | S |
| M13 | `data.fill` benzeri `fill_with_data` aracı | `engine/src/datafill.rs` | S |
| M14 | `text.fit` aracı | `engine/src/textfit.rs` | S–M |
| M15 | Şablonlardan yeni dosya | `engine/src/templates/*` | M |
| M16 | Canlı oturumda ajan imleci ve varlığı | `engine/src/live.rs:1958` | M |
| M17 | `inspect.properties`: değerlerin arkasındaki stil ve değişken adları, renk formatı | `devinspect.rs` | M |
| M18 | `component.playground`: belgeye dokunmadan prop deneme, kod ve PNG döndürme | `playground.rs` | M |
| M19 | UI sürme araçları (`pointer`, `key`, `type_text`, `inspect_ui`) ve JSON kontrol kanalı | `headless.rs`, `ui-egui/src/control.rs` | M–L |
| M20 | Headless MCP ve CLI: `Engine`'i native crate'e ayırma, `pergel-render`'ı native derleme | `headless.rs`, `apps/figcraft-cli` | L |
| M21 | Genel komut registry'si (`list_commands` / `run_command`) | `cmd/mod.rs`, `catalog.rs` | L |
| M22 | Tekrarlanabilir `perf` komutu ve sentetik doküman üreticileri | `synthetic*.rs`, `perf.rs` | M |

---

## 5. Arayüz, ikonlar, tema, dil, erişilebilirlik, kısayollar

Ölçek: FigCraft'ın egui arayüzü yaklaşık 64 bin satır (`crates/ui-egui/src`), Pergel'in Svelte
arayüzü yaklaşık 12,9 bin satır (`src/lib/ui`).

Görsel dil iki tarafta da Figma UI3'e çok yakın:

- Sekmeli başlık çubuğu
- 56 px ikon rayı
- Pages ve Layers panelleri
- Sağ panelde avatarlar, Share düğmesi, Design ve Prototype sekmeleri
- Yüzen hap şeklinde araç çubuğu

FigCraft'ın ekran görüntüleri `docs/images/shot-1…7.png`'de. Not: görüntüler eski, vurgu rengi
mavi; güncel marka rengi yeşil `#02DA96`.

### 5.1 Panel panel

| Alan | Pergel | FigCraft'ta fazladan olanlar |
|---|---|---|
| Başlık ve sekmeler (`TitleBar.svelte` / `tabs.rs`, `windows.rs`) | Sekmeler; Ctrl+T, Ctrl+W, Ctrl+Shift+T; açık ve kapatılmış sekmeler menüsü | Sekmeyi yeni pencereye koparma, çoklu pencere, sekme şeridinde tema düğmesi, sunumlar sekme olarak |
| Katmanlar (`LeftPanel.svelte` / `layers.rs`) | Küçük resimler (**FigCraft'ta yok**), sürükle-bırak, Alt+L | Alt-tık ile tüm alt katmanları açma; göz ve kilit üzerinde sürükleyerek çok satırı değiştirme; F2 ve Tab ile sonraki satırı yeniden adlandırma; regex ve numaralı toplu yeniden adlandırma; ok tuşları ve Shift+F10; sürüklerken hover edilen kapsayıcının açılması; Pages/Layers ayırıcısı sürüklenebilir |
| Design paneli (`DesignPanel.svelte` / `right.rs` …) | Hizalama, konum, auto layout, dolgu, kontur, efekt, ızgara, dışa aktarma, notlar, seçim renkleri | Vektör nokta paneli; Scale paneli; Section paneli; selection spacing tutamaçları; alanlarda hover'da değişken düğmesi; dolgu/kontur/efekt listesini sürükleyerek sıralama (`list_drag.rs`); font seçici (arama, kendi fontuyla önizleme, son kullanılanlar, hover'da tuvalde canlı önizleme); Type settings penceresi (OpenType, eksenler, kırpma, girinti) |
| Assets (`assets_panel.rs`) | Basit liste | Sayfa, frame ve `/` adlarına göre klasörler; sanallaştırılmış küçük resimler; arama; Alt+bırak ile swap; stiller; kütüphane yayımlama, etkinleştirme, değiştirme |
| Variables (`VariablesDialog.svelte` / `variables_panel.rs`) | Modal tablo | Kütüphane koleksiyonları salt okunur; tablodan alias yapma; JSON ve CSS import/export |
| Prototype | `Interactions.svelte` | Akış başlangıçları; overlay ayarları; if/else ifade editörü (`proto_editor.rs`); tuvalde "+" tutamağından bağlantı sürükleme (`proto_canvas.rs`) |
| Dev Mode | RightPanel'de tek katman için kod | Ayrı Inspect sekmesi, Ready for dev listesi, Focus view, kategorili notlar, dev resources, kalıcı ölçümler, sürüm karşılaştırma (yan yana ve kaydırıcılı) |
| Yorumlar (`CommentPins.svelte` 229 satır / `comments.rs` 1.197 satır) | Pin, konu, yanıt | Pin kümeleme, pin sürükleme, mesaj düzenleme ve silme, emoji tepki, @mention, okundu bilgisi, link, Comments paneli (arama, filtre, sıralama), sunumda yorum |
| Sürümler | Modal | Kenar çubuğunda; tuvalde salt okunur önizleme ve şerit; Rename, Duplicate, Delete; dal listesi |
| Ana ekran (`Home.svelte` / `home.rs`, `home_recents.rs`) | Recents, Drafts, Starred, Trash (**FigCraft'ta yok**), arama, ızgara ve liste | Templates sekmesi (8 şablon); "Getting started" kartı (6 ipucu, kısayollar keymap'ten); kodla oturuma katılma; klavye ile gezinme; "Show in folder" |
| Sunum (`Present.svelte` 444 satır / `present.rs` 3.320 satır) | Ekran yakalama ve CSS geçişleri | Tuvalde yüzen önizleme (Shift+Space), cihaz çerçeveleri, responsive, gerçek smart animate, oyun kolu |
| Araç çubuğu (`Toolbar.svelte` / `toolbar.rs`) | ContextBar (**FigCraft'ta yok**) | Shape Builder; Draw/Design/Prototype/Inspect mod değiştirici; "?" Yardım; vektör düzenleme araç çubuğu (Bend, Paint bucket, Lasso, Cut, Eraser, Variable width, "Done"); Enter ile şekli görünümün ortasına koyma |
| Ana menü | Düz, yaklaşık 20 öğe; geliştirici öğeleri karışık | 12 menü, 245 satır, katalogdan üretiliyor (`catalog.rs`): File 27, Edit 33, View 40, Object 59, Vector 6, Text 27, Arrange 15, Tools 18, Plugins 3, Libraries 3, Preferences 10, Help 4. Açık seçeneklerde onay işareti; değer isteyen öğeler önce diyalog açıyor |
| Menü davranışı (`Menu.svelte` / `menus.rs`) | Hover'da alt menü hemen açılıyor, çapraz geçişte kayboluyor | Alt menü açılma gecikmesi 0,15 s, devir 0,3 s, **güvenli üçgen**; klavye ve Shift+F10 |
| Diyaloglar | Tercihler, Hakkında, kısayol paneli, toplu yeniden adlandırma **yok** | About ve katkıda bulunanlar; **Preferences** (tema, dil, boyut, hareket azaltma, nudge miktarı, autosave, "Start on Home", yorum adı); Missing fonts; genel Import report; büyük dosya açılırken bilgi kartı |

### 5.2 İkonlar

| | Pergel | FigCraft |
|---|---|---|
| Kaynak | `@lucide/svelte`, yaklaşık 120 ikon ve birkaç satır içi `<svg>` | `crates/ui-egui/src/icons.rs` (1.451 satır): 158 ikon, tamamı kodla çizilmiş ve özgün (MIT/Apache) |
| Izgara | Lucide 24 px, strokeWidth 1,6–2, boyut 10–17 px | 16×16 piksel ızgarası, çizgiler pikselin ortasında, küçük boyutta keskin; araç ikonlarının aynı kalınlıkta olması testle doğrulanıyor |
| Figma'ya özgü glifler | Eksik ya da yaklaşık: Frame için `Hash`, Section için `PanelTop`, Line için `Slash`, Polygon için `Triangle` | Var: CornerTopLeft…, IndependentPadding, LayoutWrap, Hug, FillContainer, Padding*, Gap, ClipContent, AbsolutePosition, Slot, ShapeBuilder, VariableWidth, Arc*, GamutWarning, Board, Marker, Stamp |
| Galeri | Yok | `icons::gallery()` |
| İmleçler | CSS anahtar kelimeleri | `cursors.rs`: köşeye göre dönen rotate imleci; pen durumları (yeni nokta, kapatma, ekleme, bükme); damlalık; yorum; kopyala; izin yok |
| Uygulama ikonu | `app-icon.svg` | Bukalemun; `xtask/src/icon.rs` resvg ile PNG, ico ve hicolor üretiyor. **Marka ikonu, kullanılmamalı.** |

**Öneri:**

- `src/lib/ui/icons/` altında 16 px'lik 25–30 özel SVG ikon çizmek. Bunlar Figma'ya özgü glifler: Frame, Section, Hug/Fill/Fixed, padding kenarları, bağımsız köşeler, wrap, clip, shape builder, variable width, bend, paint bucket, lasso. FigCraft'ın geometrisi lisans uygun olduğu için SVG path'e elle çevrilebilir. **M**
- SVG data-URI imleçleri. **S–M**

### 5.3 Tema ve boyut

- **Pergel:**
  - `src/app.css`'te yaklaşık 20 token var.
  - Yalnızca koyu tema.
  - Bileşen stillerinde **215 sabit renk değeri** var.
  - Arayüz boyutu ayarı yok; paneller sabit 240 px.
- **FigCraft** (`theme.rs`):
  - **38 token**, `Tokens::light()` ve `Tokens::dark()`.
  - View ▸ Theme altında Light, Dark, Follow system.
  - WCAG kontrastı:
    - Dolu yüzeylerde marka rengi `#02DA96`.
    - Beyaz üstünde ince çizgi ve metin için `#00875F` (4,5:1).
    - Yeşil zeminde yazı için `#07352A`.
    - `theme::tests` her iki temada 3:1'i kontrol ediyor.
  - Koyu temada yeni dosyaların sayfası `#1E1E1E`, açık temada `#F5F5F5`.
  - Hareket azaltma.
- **Arayüz boyutu** (`ui_scale.rs`):
  - %75–200.
  - Pencere küçükse geçici olarak sınırlanıyor.
- **Paneller sürükleyerek boyutlanıyor:**
  - Sol 200–480 px, sağ 240–480 px.
  - Çift tık varsayılana döndürüyor.
  - Ölçüler tercihlerde saklanıyor.

### 5.4 Yerelleştirme (i18n)

**Pergel'de i18n yok.** 500'den fazla İngilizce etiket kodun içinde dağınık.

**FigCraft'ta 19 dil var** (`crates/ui-egui/locales/*.toml`): bn, cs, de, en, es, es-419, fi, fr,
id, ja, ka, ko, nl, pl, pt-BR, ru, sv, **tr**, zh-Hans.

- **`tr.toml`:** 2.365 satır, toplam 2.272 girdi (`[ui]` yaklaşık 1.680, `[commands]` 561, `[menu]` 31). Başlıkta makine taslağı olduğu ve ana dili Türkçe olan biri tarafından gözden geçirildiği yazıyor. Kurallar da belirtilmiş: düğmelerde yalın emir ("Kaydet"), yüzde işareti başta (%50).
- **Mekanizma** (`i18n.rs`, 245 satır):
  - Anahtar İngilizce metnin kendisi: `tr("Fill")`, yer tutuculu `trf("Delete {name}")`, çoğullu `trn("{n} layers", n)`.
  - Aynı İngilizce metnin iki anlamı varsa bağlam ekleniyor: `"Fill|sizing"`.
  - Çeviri yoksa İngilizce gösteriliyor; ham anahtar asla görünmüyor.
  - Dil yeniden başlatmadan değişiyor; sistem dili okunuyor.
  - Testler eksik anahtarları ve yer tutucu uyuşmazlıklarını yakalıyor.

**Öneri:**

- `src/lib/i18n.ts`: İngilizce metin anahtar, JSON katalog, `Intl.PluralRules`.
- FigCraft `tr.toml`'daki aynı İngilizce etiketleri otomatik eşleştiren bir betik (lisans atfıyla). Figma terminolojisi aynı olduğu için etiketlerin büyük kısmı doğrudan eşleşir.
- Altyapı **M**; bütün metinlerin geçişi **L**.

### 5.5 Erişilebilirlik, RTL, CJK, IME

| | Pergel | FigCraft |
|---|---|---|
| Ekran okuyucu | DOM'dan bedava: 41 `aria-label`, bazı `role`'ler. Tuval okunamıyor | `a11y.rs` (1.022 satır), AccessKit ağacı; gerçek ekran okuyucuyla henüz test edilmemiş |
| Klavye | Yalnızca standart Tab | F6/Shift+F6 ile bölgeler arası geçiş; menülerde ok tuşları; Shift+F10; F8/Ctrl+Space ile klavyeyle kutu seçimi; odak halkası tokenı |
| Hareket azaltma, kontrast testi | Yok | Var |
| RTL ve CJK | WebView'den **bedava** | `bidi.rs` ve `cjk.rs` ile elle (yaklaşık 500 satır) |
| IME | Yerel textarea; **bedava ve daha iyi** | Elle; macOS'ta doğrulanmamış |

### 5.6 Kısayollar

- **FigCraft:** tek kaynak `crates/engine/src/keymap.rs` (552 satır).
  - Her bağlama şunları taşıyor: grup, komut id'si, mac tuşu, win/linux tuşu, bağlam (Canvas, Text).
  - Menü, tuval, kısayol paneli ve MCP `key` aracı bu tabloyu okuyor.
  - `shortcut_parity_tests.rs` belgelenmiş 129 Figma kısayolunu iki platformda test ediyor.
  - Tuşlar klavye düzeninde yazılan karaktere göre eşleniyor.
- **Kısayol paneli** (`shortcuts_panel.rs`, Ctrl+Shift+?): 12 sekme, arama, platforma göre tuş görselleri.

**FigCraft'ta olup bizde olmayan kısayollar:**

| Grup | Kısayollar |
|---|---|
| Genel | Ctrl+/ (Actions), F11 (tam ekran), Ctrl+, (Tercihler), Ctrl+Shift+S (Save as), `/` (cursor chat) |
| Araç | Shift+M (Shape builder) |
| Görünüm | Shift+' (piksel ızgarası), Ctrl+Shift+O (outline modu), Ctrl+Alt+P (piksel önizleme), Shift+C (yorumlar), Alt+1…9 (paneller), Shift+E (Design ile Prototype arası), Ctrl+Shift+\ (kenar çubuğu) |
| Yakınlaştırma | Ctrl+0, değiştiricisiz `=` ve `-`, Home/End (önceki ve sonraki frame) |
| Metin | Ctrl+K (link), Ctrl+Shift+8 ve 7 (listeler), Ctrl+] ve [ (girinti), Ctrl+Shift+. ve , (boyut), Ctrl+Alt+. ve , (kalınlık), Alt+Shift+. ve , (satır yüksekliği), Alt+. ve , (harf aralığı) |
| Şekil | Alt+Shift+U, S, I, E (boolean), Ctrl+Shift+J (yumuşak birleştirme), Shift+Delete (sil ve onar) |
| Seçim | `\` (üst katman), F2 (yeniden adlandır), F8 (klavyeyle kutu seçimi) |
| Düzenleme | Ctrl+Shift+V (yerinde yapıştır), Ctrl+Shift+E (dışa aktar), Ctrl+Shift+I (içe aktar) |
| Dönüşüm | Alt+R (dönme merkezi) |
| Component | Ctrl+Alt+Shift+K (ana bileşene git), Shift+I (bileşen ekle) |
| Prototip | Shift+Space (tuvalde önizleme) |
| Hareketler | Z basılıyken zoom aracı; seçim yokken oklarla görünümü kaydırma; Ctrl+oklar ile boyutlandırma; Ctrl basılıyken snap'i askıya alma; **iki rakamla opaklık** (4 sonra 5 → %45) |

### 5.7 Sayı alanları

- **Pergel** (`NumberField.svelte`):
  - Aritmetik ifade var, ama `Function()` ile değerlendiriliyor; CSP sıkılaşırsa kırılır.
  - Göreli düzenleme yok: `+10` yazınca değer 10 oluyor.
  - Birim yazımı yok, Alt ile ×0,1 yok, Tab ile kaydedip ilerleme yok, Mixed alanda katman başına göreli düzenleme yok.
- **FigCraft** (`field_math.rs`, `widgets.rs`):
  - `Typed::Value | Rel(Add|Mul|Div)`, parantez, birimler (`px`, `°`, `%`, `ms`), ondalık virgül.
  - Etiket sürüklerken Shift ×10, Alt ×0,1; ekran kenarı aşılınca da devam ediyor.
  - Tab ile kaydedip ilerleme, Esc ile geri alma.
  - Mixed alanda her katmana kendi değerinden göreli düzenleme, tek undo adımında.
  - Odaklanınca tüm metin seçiliyor.
  - Hex alanı CSS renk adı kabul ediyor (`red`).
  - Testler: `field_tests.rs`, 688 satır.

### 5.8 Arayüz iş listesi

| # | İş | FigCraft referansı | Zorluk |
|---|---|---|---|
| U1 | **Tek keymap** (`src/lib/keymap.ts`): CanvasView'daki if zincirini kaldır; menüler, komutlar ve `title` metinleri buradan okusun (D11) | `keymap.rs` | M |
| U2 | Kısayol paneli (Ctrl+Shift+?) | `shortcuts_panel.rs` | S (U1'den sonra) |
| U3 | Eksik kısayollar (5.6) | `keymap.rs` | S–M |
| U4 | Açık, koyu ve sistem teması: 215 sabit rengi tokena çevir, FigCraft token değerlerini uyarla | `theme.rs` | M |
| U5 | Arayüz boyutu (Tauri `setZoom`; tuval zoom'uyla çakışmamalı) | `ui_scale.rs` | S |
| U6 | **i18n iskeleti ve Türkçe** (`tr.toml` eşleme betiği) | `i18n.rs`, `locales/tr.toml` | M (iskelet), L (tam geçiş) |
| U7 | Tercihler penceresi | `dialogs.rs` | M |
| U8 | Tam ana menü (katalogdan üretilen; geliştirici öğeleri ayrı alt menüye) | `catalog.rs`, `menus.rs` | M |
| U9 | Menüde gecikme ve güvenli üçgen | `menus.rs`, `menu_aim_tests.rs` | S |
| U10 | Menülerde klavye ve Shift+F10 | `menus.rs` | S–M |
| U11 | F6 bölge gezinmesi, `:focus-visible`, `prefers-reduced-motion` | `a11y.rs` | M |
| U12 | Kontrast testi (vitest) | `theme::tests` | S |
| U13 | Gelişmiş sayı alanı; `Function` yerine küçük bir ayrıştırıcı | `field_math.rs` | S (Mixed için M) |
| U14 | Hex alanında CSS renk adı | `color/src/named.rs` | S |
| U15 | Odaklanınca tüm metni seç | `widgets.rs` | S |
| U16 | Sürüklenerek boyutlanan paneller | `panel_width_tests` | S |
| U17 | Fill, stroke, effect ve grid listelerini sürükleyerek sıralama | `list_drag.rs` | S–M |
| U18 | Font seçici popover'ı (arama, önizleme, son kullanılanlar, canlı tuval önizlemesi) | `typography.rs`, `font_preview.rs` | M |
| U19 | Stil seçicide hover'da canlı önizleme | `style_picker.rs` | S–M |
| U20 | Toplu yeniden adlandırma diyaloğu | `dialogs.rs`, `engine/src/rename.rs` | M |
| U21 | Missing fonts diyaloğu (toast yerine) | `missing_fonts.rs` | S |
| U22 | Ortak import report diyaloğu (SVG ve sürükle-bırak için de) | `import_report.rs` | S |
| U23 | Ana ekranda Templates, Getting started, klavye, "Show in folder" | `home.rs`, `home_recents.rs` | M |
| U24 | Büyük dosya açılırken bilgi kartı | `pending_open.rs` | S |
| U25 | Araca ve tuşa göre imleçler | `cursors.rs` | S–M |
| U26 | Mod değiştirici ve "?" Yardım menüsü | `toolbar.rs` | S |
| U27 | Vektör düzenleme araç çubuğu ve "Done" | `toolbar.rs` | M–L (motora bağlı) |
| U28 | Figma'ya özgü 16 px ikonlar | `icons.rs` | M |
| U29 | Hakkında ve katkıda bulunanlar penceresi | `credits.rs` | S |
| U30 | Kenar çubuğunda sürüm geçmişi ve salt okunur önizleme | `versions.rs` | M |
| U31 | Yorum paneli, tepkiler, mention, kümeleme | `comments.rs` | M–L |
| U32 | Assets'te klasörler, stiller, Alt ile swap | `assets_panel.rs` | M |
| U33 | Sekmeyi koparma ve çoklu pencere | `windows.rs`, `tabs.rs` | L |
| U34 | Oyun kolu ile sunum (tarayıcı Gamepad API) | `apps/figcraft/src/gamepad.rs` | S |

---

## 6. Canvas çekirdeği

### 6.1 Vektör ağları ve vektör düzenleme

| Özellik | Pergel | FigCraft |
|---|---|---|
| Veri modeli | Alt yollar (anchor + cin/cout); SVG `d` string olarak saklanıyor (`vector_edit.rs`, `paint.rs:VectorPath`) | Gerçek ağ: `VectorNetwork{vertices, segments, regions}` (`crates/doc/src/vector.rs`) |
| Dallanma (bir noktada 2'den fazla segment) | Yok | Var |
| Paint bucket ve bölgeler | Yok | Var: half-edge ile yüz çıkarma (`vector_faces.rs`, `vector_regions.rs`) |
| Nokta başına köşe yarıçapı | Yok | Var (`Vertex.radius`, `rounded()`) |
| Aynalama modları | Alt'a basılıysa serbest, değilse aynalı | None / Angle / AngleAndLength |
| Nokta başına cap ve join | Yok | Var |
| Nokta silme | Eğri korunmuyor | **Heal**: eğri silinen noktadan geçmeye devam ediyor (`vectoredit/ops.rs:400`) |
| Bend, Lasso, Cut, Eraser | Yok | Var (`ops.rs`) |
| Seçili noktalar için transform kutusu | Yok | Var (`vectoredit/transform.rs`) |
| Nokta snap'i | Yok | Var (`point_snap.rs`) |
| Birden çok vektörü birlikte düzenleme | Yok | Var (`begin_many`) |
| Nokta nudge, X/Y alanları, tümünü seç, yolu ters çevir | Yok | Var |
| Pen ile açık yolu sürdürme ve dallandırma | Yok (yeni alt yol açılıyor) | Var |
| Simplify | RDP + Catmull-Rom | kurbo eğri uydurma |
| Offset path | **Daha iyi**: miter/round/bevel seçimi, yeni katman olarak | Yalnızca round, katmanı yerinde değiştiriyor |

### 6.2 Boolean ve shape builder

- **Pergel'in boolean'ı yıkıcı.** Sonuç tek vektör oluyor, operandlar siliniyor, metin desteklenmiyor (`main.rs:3136`).
- **FigCraft'ta canlı boolean grubu var.** `NodeKind::BooleanOp`:
  - Operandlar düzenlenebilir çocuklar olarak kalıyor ve sürükleme sırasında sonuç güncelleniyor.
  - Operand başına ayrı işlem seçilebiliyor.
  - Prefix fold önbelleği var (`render/src/boolean.rs`).
  - Çıkarılan deliğe tıklama arkadaki katmana geçiyor (`engine/src/booleans.rs`).
- **Shape builder** (`engine/src/shapebuilder.rs`, pathops `faces()`):
  - Seçili şekiller düzlemi arrangement yüzlerine bölüyor; stroke'lar ve açık yollar da katılıyor.
  - Tıklayınca birleştiriyor, Alt ile kaldırıyor.
- **Pergel'e taşıma:**
  - Canlı boolean: Skia PathOps ile fold + önbellek. **M**; motor kısmı bedava.
  - Shape builder: linesweeper ile **M**, yalnızca PathOps ile **M–L**.

### 6.3 Çizim araçları ve stroke

| Özellik | Pergel | FigCraft |
|---|---|---|
| Kalem basıncı | Yok | Var; genişlik profiline yazılıyor |
| Fırçalar (stretch ve scatter, 8 hazır ayar) | Yok | Var (`pathops/src/brush.rs`) |
| Stroke sayısı ve boyası | **Tek stroke, yalnızca düz renk** | Stroke listesi (gradient, image, pattern) |
| Kesik çizgi | `[dash, gap]` | Rastgele dizi |
| Cap ve join | Yalnızca vektörde, iki uca tek cap | Başlangıç ve bitiş ayrı; miter limiti |
| **Değişken genişlik** | Yok | `WidthProfile` (Fritsch–Carlson monoton kübik), hazır ayarlar (Wedge, Taper, Eye, Rounded, Pinch), tek taraflı genişlik, ⇧W Width aracı (`props.rs:200-420`, `pathops/src/width.rs`, `vectoredit/width.rs`) |

**Değişken genişliğin taşınması:**

- Skia'da yerleşik yok.
- `width.rs` saf kurbo olduğu için uyarlanabilir; ya da aynı algoritma Skia `ContourMeasure::pos_tan` ile yazılabilir.
- Zorluk:
  - Profil ve hazır ayarlar: S
  - Outline: M
  - Width aracı: M
  - Fırçalar: M

### 6.4 Parametrik şekiller

| Şekil | Pergel | FigCraft | Taşıma |
|---|---|---|---|
| **Corner smoothing (squircle)** | Yok | `geom/src/lib.rs:30-137`: Figma'nın yayımlanmış eğrisi (iki kübik + yay) | Yaklaşık 110 satır saf matematik. Dokunulacak yerler: `shape_path`, clip, hit-test, gölge. **S–M** |
| Yay, pasta dilimi, halka | Yok | `ellipse(w,h,start,sweep,inner)`, tuvalde handle'lar (`arc_handles.rs`) | Skia `add_arc` ile bedava. Handle'lar **M** |
| Parametrik poligon ve yıldız | Oluşturulurken vektöre dönüşüyor; sonradan değişmiyor | `Polygon{count}`, `Star{count, inner_ratio}`, köşe yuvarlama, handle'lar (`shape_handles.rs`) | Model **S**, handle'lar **M** |

### 6.5 Metin

| Özellik | Pergel (SkParagraph) | FigCraft | Skia ile çözüm | Zorluk |
|---|---|---|---|---|
| Satır kırma | **Tam ICU** | UAX#14'ün alt kümesi | – | (Pergel üstün) |
| Renkli emoji | **COLRv1 dahil** | COLRv1 yok | – | (Pergel üstün) |
| Düzenleme | **Yerel textarea**: IME, yazım denetimi | Tuvalde kendi caret'i, WYSIWYG | SkParagraph `get_rects_for_range` ile WYSIWYG mümkün, ama textarea korunmalı | L |
| Max lines ve ellipsis | Yok | Var | `set_max_lines` + `set_ellipsis`, **bedava** | S |
| Dekorasyon stili, kalınlık, renk, skip-ink | Yok | Var | `set_decoration_style`, `_thickness_multiplier`, `_color`, `_mode(Gaps)`, **bedava** | S |
| Gradient ve image metin dolgusu | Yok | Var | `set_foreground_paint(shader)` | S |
| Tüm değişken font eksenleri | Yalnızca `wght` | Run başına her eksen, adlandırılmış instance'lar, canlı önizleme (`textpreview.rs`) | `FontArguments::set_variation_design_position` | S motor + M UI |
| Leading trim (cap height) | Yok | Var | `metrics().cap_height` | S |
| Small caps | Yok | Var | `smcp` özelliği | S |
| Paragraf aralığı ve girintisi | Yok | Var | Paragrafları ayrı layout'la istiflemek ya da placeholder | M / S–M |
| İç içe listeler (8 seviye, başlangıç numarası, asılı işaret) | Düz; işaret metne gömülü (`text.rs:330`) | Var | Paragraf başına layout ve işaretin ayrı çizilmesi | M |
| Asılı noktalama | Yok | Var | Glif ölçümü | M |
| **Text on path** | **Shaping'siz** (`text_to_glyphs`; kerning, ligatür, Arapça yok), tek stil, start ve flip yok (`pergel-render/src/text.rs:236`) | Önce layout, sonra path'e eşleme; start offset; flip (`text/src/onpath.rs`) | Paragraph glif konumları + `ContourMeasure::pos_tan` + `RSXform` | M |
| `text.fit` (yetim kelime korumalı) | Yok | `engine/src/textfit.rs` | `measure()` ile ikili arama | S–M |

### 6.6 Render ve efektler

| Özellik | Pergel | FigCraft |
|---|---|---|
| Arka uç | **GPU** (Skia WebGL2) | CPU (tiny-skia), rayon ile paralel bantlar, render worker |
| **İzole katmanlar** (grup opacity, blend, efekt) | **Yok**: render düz, gruplar atlanıyor, opacity çarpılarak iniyor | Var (`draw.rs` `needs_layer`) |
| Drop shadow spread | Vektörde ve metinde yok; grup gölgesi yok; blend ve "show behind" yok | Kesin spread (elips, yıldız, metin), piksel spread, blend, show_behind (`shadow.rs`, `fx.rs`) |
| Layer blur | Var; frame'de yalnızca frame'in kendi dolgusunu etkiliyor | Var |
| Background blur | Var (GPU) | Var (CPU) |
| **Progressive blur** | Yok | `render/src/blur.rs`: geometrik aralıklı en fazla 8 seviye ve harmanlama |
| **Glass** (kırılma, dispersiyon, frost, ışık) | Yok | `render/src/glass.rs`: kesin Öklid mesafe dönüşümü (EDT), normaller, kanal başına ofset |
| **Noise / texture** | Yok | `render/src/grain.rs`: konum hash'i ile pan ve export'ta sabit |
| **Pattern dolgu** | Yok | `render/src/pattern.rs` |
| Maske | Yalnızca vektör clip; opacity ve gradient yok sayılıyor | Alpha / Vector / Luminance |
| Retained render | Yok | `retained.rs`: yalnızca açılan şeritler ve değişen bölgeler yeniden çiziliyor; rastgele yürüyüş testleri tam render ile karşılaştırıyor |
| Kaba önizleme | Yok | Önce 1/16 piksel (`render_worker.rs`) |
| **Mekânsal indeks** | **R-tree** | Yok |
| Renk profili | Yalnızca sRGB | Belge sRGB ya da P3, ICC v4, PNG cICP, image ICC (moxcms). Ekranda P3 **yok** |
| Outline modu, piksel önizleme, rasterize | Yok | Var |
| Vektör export preflight | Yok | `export_depth.rs` |

**Pergel'e taşıma (Skia, GPU):**

1. **Ağaç yürüyüşlü renderer (önkoşul, M).**
   - `draw_detailed` (`pergel-render/src/lib.rs:223-396`) düz çiziyor.
   - Yalnızca `needs_layer` doğru olan alt ağaçlar `canvas.save_layer(paint{alpha, blend, image_filter})` ile çizilmeli; geri kalanı bugünkü hızlı düz yolda kalmalı (hibrit).
   - Bu yapılınca hemen açılanlar:
     - Grup ve frame opacity, blend, blur
     - Alpha maske (DstIn) ve luminance maske (`color_filters::luma` + DstIn) (S)
     - Grup ve metin gölgesi (`image_filters::drop_shadow_only` + `dilate/erode`)
     - Pattern dolgu (`PictureRecorder` + `Picture::to_shader(Repeat)`, neredeyse bedava)
2. **Noise ve texture (S–M):** SkSL runtime shader + katman yerel koordinatında integer hash; texture için `image_filters::displacement_map`.
3. **Progressive blur (M):** SkSL runtime image filter; N bulanık girdi arasında lerp. Background için backdrop filtresine verilir.
4. **Glass (rect, rrect ve elips için analitik SDF ile M; rastgele şekil için L):** SkSL'de backdrop + SDF + normal + kırılma + ışık. Pergel bunu GPU'da FigCraft'tan hızlı yapar.
5. **Vektörde gölge spread'i (S):** `fill_path_with_paint(stroke 2×spread, Miter)` + PathOps.
6. **P3 (S–M; Pergel burada FigCraft'ı geçebilir):**
   - Skia yüzeyi `ColorSpace` ile oluşturulur; WebGL2'de `drawingBufferColorSpace = "display-p3"` ile **ekranda gerçek P3** elde edilir.
   - Image ICC'yi Skia codec'leri bedava okuyor.
   - Renk seçicide gamut uyarısı M.

### 6.7 Boyalar

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Diamond gradient | **Hata** (D5) | Var | S |
| 3 handle'lı (skew'li) gradient | Skaler genişlik | `Gradient.width: (f64, f64)` (`color/src/lib.rs:255`) | S |
| Gradient stroke ve metinde | Yalnızca dolguda | Dolgu, stroke ve metin | M (stroke listesi modeliyle) |
| Dolgu başına blend ve opacity, sürükleyerek sıralama | Blend yok | Var | S |
| Image crop matrisi ve 90° döndürme | "Crop" aslında esnetme | `ImagePaint.crop: [f64;6]`, rotation | S–M |
| Image hazır ayarları | Yok | 6 hazır ayar | S |
| Animasyonlu GIF | Yok | İlk kare; sunumda oynuyor | S |
| Renk seçici | Hex, RGB, HSL, HSB, eyedropper, belge renkleri | Ayrıca CSS ve isimli renkler, P3 gamut uyarısı, boya başına blend | S–M |

### 6.8 Snapping, ölçüm, kılavuzlar

| Özellik | Pergel | FigCraft (dosya) | Zorluk |
|---|---|---|---|
| Boyutlandırırken snap | Yok | `snap.rs` `snap_rect` | S–M |
| Eşit aralık snap'i ve etiketleri | Yok | `snap.rs:161-237` | M |
| Piksel snap'i; geometri, nesne ve piksel için aç/kapa | Yalnızca bırakınca yuvarlama | `round_move`, `view.snap*` | S |
| Döndürülmüş frame uzayında snap | Yok | `gather_in(Affine)` | M |
| Ölçüm | Seçimden hover edilene | İç içe katman (Cmd+Alt), ebeveyne, guide'a, sürüklerken canlı mesafe, Shift+Alt ile padding (`distance.rs`) | S–M |
| Frame guide'ları | Yalnızca sayfa guide'ları | Frame koordinatında, frame ile dönüyor (`guides.rs`) | M |
| Dönme merkezi (Alt+R) | Yok | `rotation_origin.rs` | S–M |

### 6.9 Dönüşüm modeli

- **Pergel:** dünya koordinatında bounds + merkez etrafında rotation. Skew ve gerçek ayna ifade edilemiyor (D6).
- **FigCraft:** `Node.transform: Affine`, ebeveyne göre.
- **Affine'e geçiş L.** Etkilenen yerler: R-tree aabb, layout, hit-test, SVG ve Figma import. Kısa vadede flip bayrakları yeterli.

### 6.10 Performans mimarisi önerileri (sırayla)

1. **Node `version` ve `dirty` kümesi** (S). Aşağıdakilerin hepsinin anahtarı bu.
2. **Pahalı alt ağaçlar için GPU texture önbelleği** (M). Efektli, blur'lu ve maskeli düğümler için.
3. **Üst seviye frame başına `Picture` kaydı** (M). Komut kodlama maliyetini düşürür; 100 bin katmanda 22 ms'nin büyük kısmının bu olduğu tahmin ediliyor, ölçerek doğrulanmalı.
4. **Kaydırmada önceki kareyi yeniden kullanma** (M–L). Efekt yoğun sahneler için.
5. **LOD inceltmesi** (S). Metni satır başına çubuk olarak çizmek.
6. **Render'ı worker'a taşıma** (L). Şimdilik gerekli değil.

### 6.11 Canvas iş listesi (özet)

**Hızlı düzeltmeler (S):**

- Diamond gradient
- Vektörde gölge spread'i
- Flip bayrakları
- Max lines ve ellipsis
- Dekorasyon stilleri
- Gradient metin
- Tüm değişken font eksenleri
- Small caps ve leading trim
- Squircle
- Yay ve halka
- Parametrik poligon ve yıldız
- Boyutlandırırken snap ve piksel snap'i
- kurbo ile eğri uydurma (pencil ve simplify)
- Nokta silmede heal, bend, lasso, nudge, aynalama modları
- Dolgu başına blend
- Image hazır ayarları

**Orta (M):**

- Ağaç yürüyüşlü renderer, ardından alpha/luminance maske, grup efektleri, pattern ve grup gölgeleri
- Noise ve texture
- Progressive blur
- P3 ve ICC
- Canlı boolean grupları
- Değişken genişlik ve Width aracı
- Nokta transform kutusu ve point snap
- Stroke listesi (gradient ve image stroke)
- Paragraf aralığı ve iç içe listeler
- Shaping'li text on path
- Eşit aralık snap'i
- Frame guide'ları
- Parametrik şekil handle'ları

**Büyük (L):**

- Vektör ağı veri modeli, ardından shape builder ve paint bucket
- Tam affine dönüşüm
- Rastgele şekiller için glass
- WYSIWYG metin düzenleme
- Retained render

---

## 7. Tasarım sistemi

### 7.1 Auto layout, constraints, layout grid

| Özellik | Pergel | FigCraft | Pergel'e taşıma (taffy) |
|---|---|---|---|
| Yatay, dikey, yatay wrap, 3×3 hizalama, baseline, Hug/Fill/Fixed, absolute, suggest | Var | Var | – |
| **Min/max genişlik ve yükseklik** | **Yok** | `ChildLayout.min_w…` (`doc/src/props.rs:1135`), `clamp_size`, `distribute` (`layout/src/lib.rs`) | Taffy `min_size`/`max_size` hazır; Node alanı, UI, Figma import ve codegen gerekiyor. **S–M** |
| Space-evenly, space-around | Yok | Var | Taffy'de hazır. **S** |
| Wrap satır hizası (align-content) | Hep FLEX_START | `ContentAlign` | Taffy'de hazır. **S** |
| Dikey wrap | Yok | `wrap: bool` yönden bağımsız | `Flow::Wrap` yerine `wrap: bool` (dosya göçü gerekiyor). **S** |
| **Grid iz tipleri** (fixed, hug, N·fr), hücre sabitleme, span, hücre içi hiza | Yalnızca `repeat(n, fr(1))` | `GridTrack`, `grid_column/row/_span`, `CellAlign`, `layout.setTrack` | Taffy `TrackSizingFunction` ve `Line{span}` hazır. **M** |
| Negatif gap | `gap.max(0)` | Var | Taffy desteklemiyor; negatif margin ile taklit. **M** |
| Strokes included | Yok | Var | Padding'e iç stroke eklemek. **S** |
| Canvas stacking (ilk çocuk üstte) | Yok | `reverse_z` | Render, hit-test, export ve codegen'de sıra çevirme. **S–M** |
| **Tuvalde padding ve gap tutamakları** | Yok | `ui-egui/src/handles.rs`, `gap_handle_tests.rs` | Overlay + engine'den bant geometrisi. **M** |
| Ok tuşlarıyla akış içinde sıralama | Yok (`pergel_nudge` yalnızca kaydırıyor) | `engine/src/al_keys.rs`: wrap'te satır değiştirme, grid'de hücre, komşu frame'e geçiş | Basit sürüm S, tam sürüm **M** |
| Sürükleyerek sıralama (wrap ve grid farkındalığı, frame'ler arası) | Kaba (`main.rs:619 flow_index`) | `al_drag_tests.rs` | **S–M** |
| Layout grid'e değişken bağlama, grid style'ları | Yok | Var | **S** |
| Test | 5 | 64 + 8 | – |

### 7.2 Component, instance, variant

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Oluşturma, instance, detach, reset, go to main, variant set, boolean/text/swap prop'ları | Var | Var | – |
| Override grupları | 12; layout tek blok, size yalnızca kökte | Yaklaşık 20: padding, gap ve align ayrı; grids, exports, interactions, iç katman size/resizing, modes, props, slot | M |
| **Slot'lar** | **Yok** | `PropDef::Slot`, `component.makeSlot`, `instance.slotInsert` (`slot_tests.rs`) | L |
| Instance swap'ta tercihli bileşenler | Yok | `InstanceSwap.preferred` | S |
| Exposed nested instances | Yok | `component.exposeInstance` | M |
| Component açıklaması | Yok | Var | S |
| Add variant, prop sırası, çakışan kombinasyon uyarısı, açık Variant PropDef | Variant isimden ayrıştırılıyor | Var | S–M |
| Variant değiştirirken override koruma | Metin ve boyut korunuyor | İsim yolu eşleşen tüm override'lar korunuyor | S–M |
| **Push changes** (instance → main) | Yok | `component.pushChanges` | M |
| Restore main | Yok | `component.restoreMain` | S |
| Component playground | Yok | `component.playground` | M |
| Hide when publishing (`.` / `_` öneki) | Yok | Var | S |
| Ölçek testi | Yok | Yaklaşık 300 component ve 2.000 instance | – |

### 7.3 Kütüphaneler

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Component yayımlama | Var | Var | – |
| **Stil ve değişken yayımlama** | Yok | `library.rs` `import_styles` / `import_variables` | M |
| Varlık bazında sürüm | Kütüphane başına tek sürüm | `LibraryManifest` | M |
| **Library swap** (A → B, isimle eşleştirme) | Yok | `engine/src/library_swap.rs` (249 satır) | M |
| Takım kütüphane sunucusu | localStorage, tek bilgisayar | `library_server.rs` + figcraft-server | L |
| Extended collections | Yok | `Collection.extends` | L |
| Kullanım sayıları | Yok | `library.usage` | M |

### 7.4 Stiller

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Text, color, effect | Var | Var | – |
| Grid style | Yok | Var | S |
| **Stil içinde değişken** (her katman kendi modunda çözüyor) | Yok | `engine/src/style_vars.rs` | M (7.5'teki frame başına mod gerekli) |
| Klasör işlemleri (taşı, kopyala, klasörü yeniden adlandır) | Kısmi | Var | S–M |
| Düzenlenince stil bağının kopması | Belirsiz | `styles_behaviour_tests.rs` | S |

### 7.5 Değişkenler

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| 4 tip, koleksiyon, mod | Var | Var | – |
| **Frame başına açık mod** (ör. bir frame Dark) | **Yok**: koleksiyonda tek global `active` (`variables.rs:215`) | `Node.var_modes`; ata zincirinde ilk seçilen mod (`instance_modes_tests.rs`) | **M (en kritik eksik)** |
| Alias | Modelde var; diyalogda oluşturulamıyor (`VariablesDialog.svelte:214`) | Tablodan alias yapma, döngü kontrolü | S |
| Mod işlemleri: kopyala, varsayılan yap, sırala | Ekle, sil, yeniden adlandır | Var | S |
| **Scope'lar** | Yok | `doc/src/styles.rs` `scopes_for` | S–M |
| Code syntax (WEB, iOS, ANDROID) | Yok | Var | S |
| **Import/export (Figma REST JSON ve CSS)** | Yok; REST importu değişkenleri de getirmiyor | `cmd/variables_io.rs` (211 satır) | S–M |
| Bağlanabilen alanlar | 12 | Ayrıca efektler, satır yüksekliği, harf ve paragraf aralığı, layout grid, gradient durakları, instance ve variant prop'ları | M |
| Hidden from publishing, token rolleri | Yok | `token_roles.rs` | S |

---

## 8. Prototipleme

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Tetikleyiciler | 3 (click, hover, after delay) | 10: ayrıca press, drag, mouse enter/leave/down/up, tuş kombinasyonu, gamepad (`doc/src/proto.rs`) | press, enter/leave, key: S–M; drag: M |
| Fare tetikleyicilerinde gecikme | Yok | `delay_ms` | S |
| **Etkileşim başına çoklu eylem** | Tek `action` | `actions: Vec` (`proto.rs:706`), sürükleyerek sıralama | M |
| **İfadeler ve if / else-if / else** | Yalnızca tek eşitlik; **UI'dan oluşturulamıyor** (`Interactions.svelte:172`) | `proto_expr.rs` (302 satır) ve `proto_expr_parse.rs` (549 satır), saf Rust ve bağımlılıksız: `and/or/not`, `+-*/`, `name[Mode]`, `mode(Coll)` | M (editör UI'ı ayrıca S) |
| Swap overlay; overlay ofseti ve arka planı; preserve scroll; scroll-to ofseti | Yok veya kısmi | Var | Her biri S |
| Set variable mode | Yok | Var | S–M (7.5'e bağlı) |
| **Easing** (13 tür, bezier, spring) | Sabit CSS `ease` | `Easing` | S (`cubic-bezier`), spring için CSS `linear()` ile S–M |
| **Gerçek smart animate** | Fade olarak oynuyor (`Present.svelte:255`) | Eşleşen katmanların tween'i, overshoot (`present.rs`) | L (sunumun katman bazlı render'a geçmesi gerekiyor) |
| Fixed ve sticky katmanlar | Yok | `ScrollPosition` | M |
| Flows ve başlangıç noktaları | Yok | `proto.flow.*` | S–M |
| Cihaz çerçevesi, döndürme, responsive | Yok | `PrototypeSettings` | M–L |
| Tuvalde bağlantı sürükleme ve çizgiye tıklayıp düzenleme | Yalnızca gösterim | `proto_canvas.rs` | M |
| Canlı önizleme paneli, sunum sekmeleri, sunumda inspect ve yorum | Yok | Var | M |

---

## 9. Dev Mode ve kod üretimi

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Dev Mode yüzeyi | Tek katmanın CSS/Tailwind'i (`RightPanel.svelte:137`) | Salt inspect canvas; düzenleme gerekçeyle reddediliyor; gruplanmış özellikler (arkasındaki stil ve değişkenle); renk formatı; px/rem (`dev_mode.rs` 1.105 satır, `devinspect.rs`) | M |
| **Anotasyonlar** | Katman başına tek düz `note` | Sabitlenmiş özellikler, renkli kategoriler, tuvalde balonlar, filtre (`annotations.rs` 471 satır) | M |
| Kalıcı ölçümler | Yok | `dev_measurements.rs` | M |
| Ready for dev | bool | ReadyForDev/Completed, "Changed" bayrağı, liste | M |
| **Compare changes** | Yok | Ready anındaki snapshot ya da bir sürümle özellik diff'i (`width 120 → 136`), yan yana veya kaydırıcılı görünüm (`handoff.rs`, `devcompare.rs`, `dev_compare.rs`) | L |
| Focus view | Yok | `dev.focus` | S–M |
| Dev resources (linkler) | Yok | `dev_resources.rs` | S |
| Kod dilleri | **React + Tailwind**, HTML/CSS | CSS, HTML, React, Leptos, SwiftUI, Compose; px/rem. Tailwind **yok** | Her yeni dil M |
| **Component kodu** (prop'lar parametre; variant union/enum, boolean flag, swap view param, slot) | Yok | `component_codegen.rs` (809 satır) | M–L |
| Code Connect | React JSX | Dil başına şablon (`{prop ? "a" : "b"}`), `figcraft.connect.json` import/export, import satırları (`code_connect.rs` 553 satır) | M |
| **Code Connect önerileri (kod tabanı taraması)** | **Var** (`code_scan.rs`) | Yok | (Pergel üstün) |

---

## 10. Sayfa, section, board, widget, datafill, şablon, yorum, sürüm, branch

| Özellik | Pergel | FigCraft | Zorluk |
|---|---|---|---|
| Sayfalar | Var | Var; ayrıca sayfa bölücüleri | S |
| Section rengi, içeriği gizleme, dev durumu | Yok | `section_panel.rs` | S |
| **Board (FigJam benzeri)** | Sticky not ve Mermaid | Ayrı dosya türü; metinli şekiller; **takip eden konnektörler** (kenara bağlanır, dirsekli, etiketli); marker, highlighter, silgi; damgalar (`engine/src/board.rs` 1.307 satır) | Toplamda L; konnektörler M, marker S |
| Widget'lar | Yok | Eklenti API 3; tıklama geri çağrısı (`widgets.rs`) | L |
| **Datafill** | Yok | `data.fill`: satır başına kopya, formatlar, export (`datafill.rs`) | S (MCP aracı olarak) |
| Başlangıç şablonları | Yalnızca slide deck | 8 şablon: mobile, landing, dashboard, design-system, wireframe, social, slides, icon-set (`templates/*`) | M (Pergel'de `create_nodes` JSON'u olarak yazmak kolay) |
| Toplu yeniden adlandırma | Yok | `rename.rs`: regex, `$1…$9`, `$n` | S |
| Selection spacing ve sıralama | Yok | `selection_spacing.rs`, `selection_reorder.rs` | M |
| Yorum tepkileri, mention, düzenleme, okundu, link, kümeleme, filtre | Yok | `doc/src/comments.rs`, `ui-egui/src/comments.rs` | M |
| Sürümde salt okunur önizleme, açıklama, rename, duplicate, delete | "Open copy" var | `versions.rs` | S–M |
| **Branch'te özellik düzeyinde merge ve çakışma çözümü** | Üst katman bazında; çakışmada son gelen kazanıyor (`merge.rs:129`) | Üç yönlü merge, her çakışmada branch ya da main seçimi, fill/stroke/effect öğe öğe; update from main, arşiv (`branches.rs`, `branch_ui.rs`) | L (bölüm 11'deki collab çekirdeğiyle ortak) |

---

## 11. İşbirliği

| Özellik | Pergel | FigCraft (dosya) |
|---|---|---|
| Canlı imleçler | Var (25 Hz) | Var; ayrıca seçim çerçeveleri, avatarlar, karşı tarafın caret'i (30 Hz) |
| **Cursor chat** | Yok | `/` ile açılıyor, 100 karakter |
| **Spotlight ve follow** | Yok | `collab.spotlight`, `collab.follow` |
| Eşzamanlı düzenleme | Tam doküman yayını, üst katman bazında merge, sunucu sıralaması yok (D1) | Özellik bazlı LWW, sunucu sıralı, frac index, red ve düzeltme (`crates/collab/src/{server,client,replica,diff,frac}.rs`) |
| Offline ve reconnect | Yok (D3) | Delta ya da snapshot, offline kuyruğu |
| İşbirliğinde undo | Bozuk (D2) | Kişi başı; yalnızca kendi adımlarını geri alıyor (`collab/src/history.rs`) |
| İzinler, kişiyi çıkarma | Yok | Görüntüle/düzenle, `collab.removePerson` |
| Davet | `ws://ip:3856/live?token=` elle kopyalanıyor | `figcraft://` URL şeması; 9 ve 20 sembollük join code'ları (`collab/src/code.rs`, `collab-net/src/invite.rs`) |
| İnternet ve TLS | Yalnızca LAN, düz `ws` | rustls; sunucu reverse proxy arkasında |
| Self-hosted sunucu | Yok | `apps/figcraft-server`: snapshot, checksum'lı log, compaction, limitler, moderasyon; Docker, Caddy, nginx, systemd, yedek |
| Tarayıcıdan katılma | Var (web modu ve BroadcastChannel) | Var (join; host yok) |
| Test | 1 tokio testi | Rastgele tohumlu yakınsama simülasyonu, kopan bağlantılar, undo ihlal denetimi (`sim.rs`) |

**Taşıma:**

| İş | Zorluk |
|---|---|
| Token, limit ve throttle (G2) | S; `throttle.rs` ve `code.rs` yalnızca std'ye bağlı, neredeyse doğrudan kopyalanabilir |
| Cursor chat, spotlight, follow, seçim paylaşımı | Her biri S; mevcut relay'e yeni mesaj tipleri |
| Görüntüle/düzenle izni | S/M |
| Deterministik tie-break (D1) | S |
| Node bazlı delta (D8) | M |
| Offline kuyruk | M |
| FigCraft tipi çekirdek | L; `figcraft_doc`'a bağlı, algoritmalar yeniden yazılmalı. `frac.rs` S ile kopyalanabilir; test için `sim.rs` modeli alınmalı |
| Self-hosted sunucu | Mevcut relay'e snapshot ve log eklemek M; FigCraft seviyesi L |
| `pergel://` URL şeması (`tauri-plugin-deep-link`) | S/M |

---

## 12. Dosya giriş/çıkış

| Format | Pergel | FigCraft (dosya) | Zorluk |
|---|---|---|---|
| **`.fig` içe aktarma** | Yok | Best effort: zip + deflate/zstd + jenerik kiwi decoder, ardından REST JSON şekline map (`crates/format/src/fig/{kiwi,map,network}.rs`) | **M; en değerli port.** `kiwi.rs` (629 satır, yalnızca std) ve `network.rs` doğrudan alınabilir; `map.rs` (3.080 satır) hafif uyarlama istiyor; sonuç Pergel'in mevcut `figma.rs` REST dönüştürücüsüne girer. Hukuki risk değerlendirilmeli (bölüm 16) |
| `.fig` dışa aktarma | Yok | Deneysel (`fig/export.rs`, 2.560 satır) | L |
| REST JSON | **Linkten canlı API ile**, görseller paralel indiriliyor, `node-id` destekleniyor; değişkenler gelmiyor | Yalnızca dosyadan; değişken JSON'u dahil | Değişken importu S–M (bölüm 7.5) |
| SVG içe/dışa aktarma | usvg | Ayrıca 22 dosyalık test corpus'u, filtreler, metin metin olarak, text on path | Corpus testleri S/M |
| PDF | Skia PDF, çok sayfalı deck | krilla: subset font, linkler | – |
| **Baskı** (bleed, crop marks, page box, CMYK) | Yok | `crates/pdf/src/print.rs` | M (CMYK için L) |
| WebP dışa aktarma | Yok | Var | S (`toDataURL("image/webp")`) |
| Animasyonlu GIF, TIFF, BMP | Yalnızca GIF içe aktarma | Var | S |
| Renk profili ve ICC | Yok | `color/src/{profile,icc}.rs` | M/L (Skia ile bölüm 6.6) |
| Çökme kurtarma | Sürekli autosave sayesinde dolaylı | `apps/figcraft/src/recovery.rs`: 10 saniyede bir pid klasörüne, kilitli | S/M (asıl kazanç D7) |
| Zip formatı | Tek JSON, base64 | `.figcraft` zip | M |
| Diğer uygulamalardan pano HTML'ini `.fig` olarak okuma | Yok | Var | M (`.fig` importundan sonra) |

---

## 13. Eklentiler

| Özellik | Pergel | FigCraft |
|---|---|---|
| API | JS, async `pergel.*` (MCP araçları) | WASM ABI v3, komut registry'si |
| Sandbox | Kendi eklentileri Web Worker'da (60 s). **Figma eklentilerinde yok** (G1) | wasmi (JIT yok), fuel bütçesi, 64 MB, WASI yok, hata olursa rollback (`crates/plugin/src/sandbox.rs`) |
| Manifest izinleri | Yok | read/write, komut izin listesi, yasaklı komutlar (`manifest.rs`, `engine/src/plugins.rs`) |
| **Figma JS eklenti uyumu** | **Var**: `figma.*` alt kümesi, `showUI`, `clientStorage`, `exportAsync` | **Yok** |
| Eklenti UI'ı | HTML iframe | egui formları; HTML yok |
| Widget | Yok | Var |
| Dizin ve dağıtım | Yok | Sunucuda dizin, SHA-256 doğrulaması (`plugin_directory.rs`, `bundle.rs`) |
| SDK | 5 JS örneği | Rust SDK + 3 örnek |

**Öneriler:**

- G1'i kapat (QuickJS sandbox).
- Manifest izinleri ve komut izin listesi (S/M). Bugün eklentiler `download_assets` dahil her MCP aracına erişebiliyor.
- Eklenti dizini (M; sunucu gerektiriyor).
- WASM eklentileri ve widget'lar (L; değeri tartışmalı, JS ekosistemi avantajımızla çakışıyor).

---

## 14. Platform, paketleme, CI, test, performans

| Konu | Pergel | FigCraft |
|---|---|---|
| Windows | x64 NSIS, imzasız | x64, x86, arm64 MSI + portable, **imzalı** (pfx ya da Azure Trusted Signing) |
| macOS | Teorik | Universal DMG, imzalı ve notarize edilmiş |
| Linux | Teorik | AppImage, deb, rpm, Flatpak, tar; x86_64, aarch64, riscv64 |
| FreeBSD | Yok | Var |
| Web | Yalnızca `npm run dev` | trunk paketi, WebGPU/WebGL2, yaklaşık 17 MB wasm |
| CI | Yalnızca tag'de Windows release | fmt, clippy `-D warnings`, test, layers, wasm32, FreeBSD, Windows ARM64, packaging-lint, SonarCloud, release |
| Test | 66 | 3.184; proptest; `tests/it`; SVG piksel corpus'u; collab simülasyonu; fuzz; `cargo xtask bench` |

FigCraft'ın CI'ı reusable workflow'lara devrediyor (`storytold/craft-common-actions@main`). İçerik
repoda değil ve SHA'ya sabitlenmemiş; örnek alırken buna dikkat.

**Performans** (doğrudan karşılaştırılamaz, ölçümler farklı):

- **Pergel (GPU):**
  - 1 bin katman 1,2 ms
  - 10 bin katman 8,9 ms
  - 100 bin katman 22 ms
  - Hit-test 1 µs'nin altında
- **FigCraft (CPU):**
  - 50 bin katman uzak zoom'da LOD ile yaklaşık 8 ms
  - 10 bin katmanda tek düzenleme yaklaşık 0,2 ms (retained)
  - Tam render 7–15 ms
  - Glass, blur ve 4K'da yavaşlıyor

**Öneriler:**

| İş | Zorluk |
|---|---|
| **PR CI'ı:** `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `npm run check` | S |
| macOS ve Linux release matrisi (tauri-action) | S/M |
| İmzalama (FC `packaging/windows/sign.ps1` ve `packaging/macos/package.sh` örnek olarak okunabilir) | M |
| SVG corpus testleri | S/M |
| clippy `unwrap_used` | S |
| Katman bağımlılık kuralı denetimi (`xtask layers` benzeri) | S |
| Web dağıtımı | M |
| FreeBSD | L; değmez, Tauri desteklemiyor |

---

## 15. Pergel'in güçlü olduğu yerler

FigCraft'tan özellik alırken bunları geriletmemeliyiz.

1. **Tasarımdan koda MCP.** `get_design_context` tek çağrıda şunları veriyor:
   - Kurallar
   - Tekilleştirilmiş varlık dosyaları ve bunları kullanan referans kod
   - Token'lar, fontlar, bileşenler
   - Anotasyonlar, Code Connect, etkileşimler
   - Ekran görüntüsü

   FigCraft'ta bu bilgiler dağınık ve varlıkları koda bağlayan bir çıktı yok.
2. **`compare_to_design`.** Uygulama ile tasarımın piksel farkı: 1 px antialias toleransı, 12×12 hücre, en kötü hücrelerin tasarım katmanlarına eşlenmesi, yüzde, yan yana görüntü. FigCraft'ta karşılığı yok; onun `dev.compare`'i tasarımı tasarımla karşılaştırıyor.
3. **Code Connect önerileri.** `code_scan.rs` React, Vue, Svelte ve cva varyantlarını tarayıp eşleme öneriyor.
4. **React + Tailwind çıktısı.** FigCraft'ta Tailwind yok.
5. **`search_icons` (Iconify, 200 binden fazla ikon) ve Icons paneli.**
6. **`generate_diagram` (Mermaid).**
7. **Figma'dan link ile canlı içe aktarma** (`node-id` dahil).
8. **Figma JS eklenti uyumluluğu.** FigCraft "Figma eklentileri çalışmıyor" diyor. G1 kapatılırsa güçlü bir farklılaştırıcı.
9. **`run_script`.** Figma `use_figma` benzeri JS, tek undo adımı.
10. **Figma Dev Mode MCP araç isimleriyle uyumluluk** ve hep açık HTTP sunucusu (tek satırlık kurulum, Agents panelinde aktivite günlüğü).
11. **GPU render ve R-tree.** FigCraft'ın en büyük açığı ("CPU canvas", gaps.md #7) bizde baştan çözülmüş. Skia'nın PathOps, image filter, runtime shader, Picture shader, color space ve PDF primitifleri bedava.
12. **SkParagraph.** Tam ICU satır kırma ve COLRv1 emoji.
13. **Web yığını.** IME, yazım denetimi, RTL, CJK ve varsayılan erişilebilirlik ağacı bedava; Svelte ile arayüz geliştirmek çok daha hızlı.
14. **Arayüzde FigCraft'ta olmayanlar:** katman küçük resimleri, ContextBar, Quick Actions'ta katman araması, ana ekranda Drafts, Starred ve Trash, Ctrl+Shift+T.
15. **Offset path.** Join seçimi var ve sonuç yeni katman olarak geliyor.
16. **Kesintisiz autosave, Slides ve Sites yayımlama** (`publish.ts`).
17. **Küçük kod tabanı.** Yaklaşık 38 bin satıra karşı 253 bin satır.

---

## 16. Lisans: neyi alabiliriz

- **FigCraft kodu:** MIT OR Apache-2.0, `Copyright (c) 2026 ArtCraft Team and the FigCraft contributors` (`NOTICE`, `LICENSE-MIT`, `LICENSE-APACHE`).
  - Pergel MIT olduğu için **MIT seçeneğiyle alınabilir**.
  - Port edilen dosyaların başına ya da bir `THIRD_PARTY_NOTICES.md` dosyasına telif satırı ve MIT metni eklenmeli.
  - Veri modelleri farklı olduğu için çoğu iş "kopyala-yapıştır" değil "uyarla".
- **Kullanılmamalı:**
  - `docs/brand/*` (ArtCraft logoları): ticari marka, `LICENSE-brand.txt`.
  - Bukalemun uygulama ikonu: lisansı uygun ama marka kimliği.
- **Rahatça alınabilecekler:**
  - `locales/tr.toml` ve diğer çeviriler (atıfla)
  - SVG test corpus'u
  - `examples/*.figcraft`
  - Kodla çizilmiş UI ikonlarının geometrisi
- **Doğrudan alınabilecek saf Rust parçalar** (az bağımlılık):

  | Dosya | Ne yapar |
  |---|---|
  | `format/src/fig/kiwi.rs`, `network.rs` | `.fig` çözme |
  | `doc/src/proto_expr*.rs` | Prototip ifadeleri |
  | `collab/src/code.rs`, `frac.rs` | Join code, kesirli sıralama |
  | `collab-net/src/throttle.rs`, `token.rs` | Throttle, token |
  | `geom/src/lib.rs` `rounded_rect`, `ellipse`, `polygon` | Squircle, yay, poligon |
  | `pathops/src/width.rs`, `brush.rs` | Değişken genişlik, fırça |
  | `ui-egui/src/field_math.rs` | Sayı alanı mantığı; TS'ye çevrilir |
  | `engine/src/rename.rs` | Toplu yeniden adlandırma |
  | `engine/src/textfit.rs` | Metni kutuya sığdırma |
  | `color/src/icc.rs`, `profile.rs` | Renk profilleri |

- **Bağımlılıklar:**
  - rustls alınırsa `webpki-roots` CDLA-Permissive-2.0 lisansını getirir.
  - `craft-libs` git bağımlılıklarının lisansı bu ağaçtan doğrulanamadı; kullanmadan önce kontrol edilmeli.
- **`.fig` formatı:** FigCraft "kamuya açık notlardan, clean-room" diyor. Figma'nın kullanım koşulları ve tersine mühendislik riski ürüne almadan önce değerlendirilmeli; REST API yolu hukuken daha temiz.

---

## 17. Yol haritası

Önerilen sıra:

1. Doğruluk ve güvenlik
2. Sonraki her şeyin üzerine kurulacağı altyapı
3. Ucuz ama görünür kazançlar
4. Derin özellikler

### Faz 0: Güvenlik ve doğruluk (1–2 hafta)

| ID | İş | Bölüm | Zorluk |
|---|---|---|---|
| G1a | Tauri capability'leri, CSP, `write_file`'ı yalnızca diyalogdan dönen yollarla sınırlama | 2.1 | S |
| G2 | Live token, bind, limitler, throttle | 2.1 | S |
| G3 | `/assets` CORS ve Host kontrolü | 2.1 | S |
| D1 | Canlı oturumda deterministik tie-break | 2.2 | S |
| D3 | Yeniden bağlanınca yerel değişiklikleri koruma | 2.2 | S–M |
| D4/M2 | MCP ve script rollback | 2.2, 4 | M |
| D5 | Diamond gradient | 2.2 | S |
| D6 | Flip bayrakları | 2.2 | S–M |
| D7 | Panic guard | 2.2 | S–M |
| D9, D10, D12 | MCP mesajları, uyarılar, kuyruk | 2.2 | S |
| CI | PR CI'ı | 14 | S |

### Faz 1: Altyapı (2–4 hafta)

| ID | İş | Bölüm | Zorluk |
|---|---|---|---|
| U1 | Tek keymap (+ U2 kısayol paneli) | 5.6 | M |
| U4 | Tema tokenları, açık/koyu/sistem | 5.3 | M |
| U6 | i18n iskeleti ve Türkçe (`tr.toml` eşleme betiği) | 5.4 | M |
| A1 | Node `version` ve dirty kümesi | 3.3, 6.10 | S |
| R1 | Ağaç yürüyüşlü renderer ve izole katmanlar | 6.6 | M |
| G1b | Figma eklentisi için QuickJS sandbox | 13 | M/L |

### Faz 2: Hızlı ve görünür kazançlar (S işler, paralel yürütülebilir)

| Alan | İşler |
|---|---|
| **MCP** | M1 undo/redo; M3 gizli araçlar; M4 export/save; M5 resource'lar; M6 prompt'lar; M7 batch + `$step`; M8 byte bütçesi; M12 stdio proxy; M13 datafill |
| **Arayüz** | U5 boyut; U9 menü gecikmesi; U13–U15 sayı alanı; U16 panel genişliği; U21, U22 diyaloglar; U24 bilgi kartı; U26 mod ve yardım; U29 Hakkında; U3 eksik kısayollar |
| **Auto layout** | Min/max; space-evenly/around; align-content; dikey wrap; strokes included |
| **Değişkenler** | Import/export (JSON ve CSS); scope'lar; code syntax; dialog'dan alias |
| **Metin (Skia'da bedava)** | Max lines ve ellipsis; dekorasyon stilleri; gradient metin; tüm değişken eksenleri; small caps; leading trim |
| **Şekil** | Squircle; yay ve halka; parametrik poligon ve yıldız |
| **Efekt** | Vektörde gölge spread'i; alpha/luminance maske (R1'den sonra); noise |
| **Snap** | Boyutlandırırken snap; piksel snap'i ve aç/kapa ayarları |
| **Vektör** | Nokta silmede heal; bend; lasso; nudge ve X/Y alanları; aynalama modları |
| **Prototip** | Conditional ve scroll-to editörü; easing; swap overlay ve ofsetler; fare tetikleyicileri |
| **Dev Mode** | Dev resources; section renkleri; toplu yeniden adlandırma; WebP dışa aktarma |
| **İşbirliği** | Cursor chat; spotlight ve follow; görüntüle/düzenle izni |

### Faz 3: Çekirdek özellikler (M)

| Alan | İşler |
|---|---|
| **Değişkenler** | **Frame başına mod**, ardından SetVariableMode ve stil içinde değişken; daha geniş bağlama alanları |
| **Prototip** | **Çoklu eylem ve ifadeler** (`proto_expr*.rs` uyarlaması); flows; cihaz çerçevesi; tuvalde bağlantı sürükleme |
| **Auto layout** | Grid iz tipleri ve span; tuvalde padding ve gap tutamakları; ok tuşlarıyla sıralama |
| **Component** | İnce override grupları; push changes; playground; tercihli bileşenler; Assets paneli yenilemesi |
| **Kütüphane** | Stil ve değişken yayımlama; library swap |
| **Canvas** | Canlı boolean grupları; değişken genişlik ve Width aracı; progressive blur; glass (rrect/elips); P3; pattern; stroke listesi; text on path; paragraf aralığı ve listeler; nokta transform kutusu ve point snap; eşit aralık snap'i; frame guide'ları |
| **Dev Mode** | Inspect yüzeyi; yapılandırılmış anotasyonlar; kalıcı ölçümler; ready-for-dev durumları; component codegen; Code Connect şablonları ve `connect.json` |
| **Dosya ve kalite** | **`.fig` içe aktarma** (hukuki onaydan sonra); zip dosya formatı; baskı PDF seçenekleri; SVG corpus testleri; MCP testleri; dosya erişim kökleri |
| **İşbirliği ve dağıtım** | Node bazlı delta; offline kuyruk; macOS ve Linux release, imzalama |
| **Arayüz** | U7 Tercihler; U8 tam menü; U11 erişilebilirlik; U18 font seçici; U23 şablonlu ana ekran; U28 ikonlar; U30 sürüm kenar çubuğu |

### Faz 4: Büyük işler (L)

| Alan | İşler |
|---|---|
| **Mimari** | Komut registry'si (`list_commands` / `run_command`, menü ve kısayollar aynı kaynaktan); Engine'i native crate'e ayırma, ardından headless MCP ve CLI |
| **Vektör ve dönüşüm** | Vektör ağı veri modeli, ardından shape builder ve paint bucket; tam affine dönüşüm |
| **Component ve prototip** | Slot'lar; gerçek smart animate |
| **Dev Mode** | Compare changes |
| **İşbirliği** | Sunucu sıralı, özellik bazlı collab çekirdeği ve branch çakışma çözümü (ortak çekirdek); self-hosted sunucu ve takım kütüphane sunucusu |
| **Board ve eklenti** | Board dosya türü ve takip eden konnektörler; widget'lar |
| **Diğer** | WYSIWYG metin düzenleme; retained render; CMYK; çoklu pencere |
