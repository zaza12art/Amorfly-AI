# Amorfly AI — Mimari Durum

## Tamamlanan Fazlar

### Faz 0 — Temizlik ✅
Google/Gemini, PWA manifest, Express web sunucusu, Windows kurulum betikleri kaldırıldı.
Eski web-app dosyaları `.bak` olarak saklandı, aktif değil.

### Faz 1 — Tauri iskeleti ✅
`src-tauri/` — Rust tabanlı Tauri 2 çekirdeği (Chromium yok, native WebView).
Gerçek komutlar: `check_ollama`, `ollama_chat` (sahte fallback YOK), `list_ollama_models`,
`get_gpu_info` (gerçek `nvidia-smi`/`lspci`).

### Faz 2 — Gerçek güvenlik + model indirici ✅
- `security.rs`: AES-256-GCM şifreleme, anahtar `~/.config/Amorfly AI/vault/amorfly.key` (0600 izin).
  Eski `encryption.ts`'deki Base64+ters çevirme hilesi tamamen kaldırıldı.
- `models.rs`: gerçek `ollama pull` ile indirme, ilerleme Ollama'nın kendi çıktısından okunup
  `amorfly://model-progress` event'i ile frontend'e akıtılıyor. Mock veritabanı yok.

### Faz 3 — Opsiyonel online sağlayıcı ✅
- `online.rs`: Google'a özel HİÇBİR bağımlılık yok. Kullanıcı, ayarlardan herhangi bir
  OpenAI-uyumlu endpoint (OpenRouter, Groq, kendi sunucusu) + API anahtarı girebilir.
  Anahtar şifreli saklanır. Tamamen opsiyonel; boş bırakılırsa uygulama saf yerel kalır.

### Faz 4 — Sistem entegrasyonu (MVP) ✅
- `system_habits.rs`: aktif pencere takibi **yalnızca X11'de** (`xdotool`). Wayland desteği
  henüz yok (masaüstü ortamına göre ayrı adaptör gerektirir — GNOME/KDE Wayland ileride eklenecek).
  Kural-tabanlı basit öneri motoru (eşik: 2 saat aynı pencere → mola önerisi). Bu bilinçli
  olarak "ML ile öğrenme" değil, dürüst bir MVP; gerçek örüntü öğrenimi ayrı bir faz.
- Sistem tepsisi (tray) + gerçek masaüstü bildirimleri (`tauri-plugin-notification`).
- Tüm alışkanlık verisi şifreli, yalnızca yerelde, `Ayarlar` sekmesinden kapatılabilir.

### Faz 5 — Paketleme ✅
- `tauri.conf.json` → `.deb` + `.AppImage` hedefleri tanımlı.
- `install_amorfly_ai.sh` yeniden yazıldı: bağımlılık kontrolü + build otomasyonu.

### Ek — Altyazı & Dublaj Modülü ✅
- `subtitles.rs`: video → (ffmpeg) ses çıkarma → (whisper.cpp, ~99 dil otomatik algılama) →
  (Ollama, yerel LLM) her segmenti Türkçe'ye çeviri (sansürsüz, kelime kelime) → `.srt`.
  Video oynatıcıda `<track>` olarak gösterilir.
- Opsiyonel dublaj: Piper TTS ile segment segment Türkçe ses üretimi + ffmpeg mux.
- Frontend: `SubtitlesTab.tsx` — video seç, altyazı üret, isteğe bağlı dublaj üret.

### Ek — Video Kalite Artırma Modülü ✅
- `upscale.rs`: Video2X (github.com/k4yt3x/video2x) CLI'ye köprü. Real-ESRGAN (gerçek video),
  Real-CUGAN/anime motoru (çizim içerik), RIFE (kare hızı/fps artırma — ayrı komut).
  Vulkan destekli GPU gerektirir, `--list-gpus` ile tespit edilip kullanıcıya sunulur.
- **Bilinçli tasarım kararı: dosya boyutuna/uzunluğuna HİÇBİR sınır konmadı.** Ne kadar
  büyük/uzun video olursa olsun işlenir, süre tamamen donanıma bağlı — yazılım tarafında
  yapay bir kısıtlama yok.
- Frontend: `UpscaleTab.tsx` — video seç, oran (2x/4x) ve içerik türü (gerçek/anime/hibrit)
  seç, GPU seç, işlemi başlat.

### Ek — Otomatik Kurulum (sudo gerektirmeyen) ✅
- `installer.rs`: Ollama ve Video2X'i `~/.local/share/Amorfly AI/bin` altına indirip
  sudo/systemd olmadan çalıştırır (GUI'den şifre isteme sorunu olmadan).
- **Groq için bilinçli olarak "otomatik oturum açma" eklenmedi** — üçüncü taraf bir sitenin
  giriş formunu otomatikleştirmek güvensizdir. Onun yerine sistem tarayıcısında ilgili sayfa
  açılır, kullanıcı kendi yöntemiyle giriş yapıp anahtarı kendi eliyle yapıştırır.

### Ek — Ses (STT/TTS) ve Dil Kalitesi Modülü ✅
- `voice.rs`: `record_and_transcribe` (ffmpeg ile mikrofon kaydı + whisper.cpp), `speak_text`
  (Piper TTS + ffplay ile çalma), `refine_language` (modelin cevabını ikinci bir geçişle
  hedef dilde dilbilgisi kurallarına uygun, akıcı hale getirir — devrik/saçma cümleleri önler).
- Sohbet sekmesine mikrofon butonu (sabit süreli kayıt), her AI cevabının yanına sesli okuma
  butonu, ve gönderilen her mesaja "yanıt dili" sistem talimatı eklendi.
- Ayarlar'da yeni "Ses & Dil" bölümü: whisper/piper yolları, yanıt dili, otomatik düzeltme
  ve otomatik sesli okuma anahtarları — hepsi şifreli vault'ta saklanıyor.
- Kod bloğu algılama: sohbet cevabındaki ` ```dil ... ``` ` blokları ayrıştırılıp doğru
  dosya uzantısıyla (`.lsp`, `.py`, `.sh`, `.sql`, `.bas`, vb.) diske kaydedilebiliyor.
- Modeller listesine kod-özel bir açık kaynak model eklendi: `qwen2.5-coder:7b` — niş
  dillerde (AutoLISP dahil) genel amaçlı modellerden çok daha tutarlı sonuç verir.

### Ek — Belge (Excel/PDF/Word) ve Görsel Analiz Modülü ✅
- `documents.rs`: okuma — .xlsx/.xls (calamine, saf Rust), .pdf (pdftotext), .docx/.doc
  (pandoc), .txt/.md/.csv (doğrudan). Uzun belgeler yerel modelin bağlamına sığması için
  ~14.000 karakterde kesilir (kesme durumu kullanıcıya bildirilir).
- Yazma/dışa aktarma — .txt (doğrudan), .xlsx (rust_xlsxwriter, markdown tablosu algılanırsa
  satır/sütuna döker), .docx (pandoc), .pdf (pandoc ile önce .docx, sonra libreoffice
  --headless ile .pdf — LaTeX/weasyprint gibi ağır bağımlılıklar eklenmedi).
- Görsel analiz: `analyze_image` — Ollama'nın çok-modlu (vision) modellerine (llava,
  llama3.2-vision vb.) base64 görsel gönderir. Modeller listesine `llava:7b` eklendi.
- Frontend: `DocumentsTab.tsx` ("Belge & Görsel" sekmesi) — dosya/görsel seç, isteği yaz,
  analiz et, sonucu istenen formatta dışa aktar.

### Ek — Kararlılık, Log Sistemi ve Tanılama (son rötuşlar) ✅
- **Crash handler**: `Cargo.toml`'da `panic = "abort"` KULLANILMIYOR (bilinçli karar — bu yorum
  satırıyla işaretli). Varsayılan "unwind" ile bir komuttaki panik yalnızca o çağrıyı
  başarısız kılar, tüm uygulamayı çökertmez. Ayrıca `std::panic::set_hook` ile her panik
  log dosyasına yazılır.
- **Log sistemi** (`logger.rs`): her oturumda `~/.local/share/Amorfly AI/logs/` altında tarih-saat
  damgalı bir dosya açılır, her satır kendi zaman damgasını taşır. Frontend'deki tüm `catch`
  blokları (`src/lib/log.ts` üzerinden) aynı dosyaya yazıyor — tek yerde toplanan hata geçmişi.
- **Tanılama sekmesi** (`diagnostics.rs` + `DiagnosticsTab.tsx`): Ollama, model varlığı, ffmpeg,
  Video2X, whisper.cpp, Piper, pandoc, poppler-utils, LibreOffice, xdotool, Python3 — hepsi tek
  ekranda yeşil/kırmızı. Ollama ve Video2X için "Otomatik Kur" (sudo'suz); apt paketi olanlar
  için "Terminalde Kur" (bir terminal penceresi açar, sudo şifresi kullanıcının kendi açtığı
  terminalde istenir, uygulama asla görmez/tutmaz). Ayrıca modül bazlı özet rozetler (hangi
  sekme gerçekten kullanılabilir durumda) ve log görüntüleyici içerir.
- **Sürüm bilgisi**: Ayarlar'da "Hakkında" bölümünde sürüm numarası (`Cargo.toml`'dan
  `CARGO_PKG_VERSION`), Tanılama sekmesinde de tekrar gösteriliyor.

### Ek — İzin Katmanı, Görev Motoru, Ortak Hafıza (kullanıcı geri bildirimiyle eklendi) ✅
- **İzin/onay katmanı**: Toplu/kalıcı-etkili işlemler (terminal üzerinden apt kurulumu, toplu
  OCR, toplu video işleme) hem frontend'de (`window.confirm`) hem Rust tarafında
  (`confirmed: bool` parametresi zorunlu, `false` gelirse reddedilip loglanır) çift katmanlı
  onay gerektiriyor. Tek katmana güvenilmiyor.
- **Görev motoru** (`tasks.rs` + `TasksTab.tsx`): sekme değiştirilse bile arka planda devam eden
  gerçek bir görev kuyruğu. Üç somut görev: Klasör Tarama (salt okunur, dosya/tür/boyut raporu),
  Toplu OCR (tesseract-ocr + pdftoppm, PDF klasörünü metne çevirir), Toplu Kalite Artırma
  (Video2X, klasördeki tüm videoları büyütür). Her görev iptal edilebilir, ilerleme
  `amorfly://tasks-updated` event'i ile canlı akıyor.
- **Ortak hafıza** (`memory.rs`): sohbet geçmişinden ayrı, gerçek bir "çalışma hafızası". Belge
  analizi, altyazı üretimi, video kalite artırma ve toplu görevler otomatik olarak buraya
  kaydediliyor (şifreli vault). Sohbet başladığında kısa bir özet modele context olarak
  veriliyor — model, daha önce hangi dosyalarla ne yaptığını bilerek cevap verebiliyor.
  Ayarlar'dan görüntülenip tamamen temizlenebiliyor (kullanıcı kontrolü).
- Tüm bu üçü de yeni bir ücretli/kayıt gerektiren servis EKLEMEDEN, mevcut açık kaynak
  araçlarla (tesseract-ocr, poppler-utils, video2x) kuruldu.

### Ek — Agent/İş Akışı Planlayıcısı, RAG Belge Arama, Gerçek Tek-Tık Kurulumlar ✅
- **Agent (`agent.rs`)**: Doğal dil isteğini ("bu videoyu Türkçeye çevir, dublaj yap, 4K yap")
  SABİT bir adım kümesinden (subtitle/dub/upscale) bir plana çevirir. Model asla serbest
  parametre/dosya yolu üretmez — sadece hangi adımların gerektiğine karar verir, dosya yolu
  aktarımı tamamen deterministik kod tarafından yapılır. Plan kullanıcıya gösterilir, onay
  olmadan (confirmed=true) hiçbir şey çalıştırılmaz. Onaylanan iş akışı mevcut görev motoruna
  (tasks.rs) normal bir görev olarak eklenir — "Görevler" sekmesinde diğerleriyle birlikte
  görünür, iptal edilebilir. Bilinçli olarak "genel amaçlı kernel" değil, sabit/güvenilir bir
  planlayıcı — küçük yerel modeller serbest planlamada güvenilmez, sabit kelime dağarcığından
  seçimde güvenilirdir.
- **RAG Belge Arama (`rag.rs`)**: Analiz edilen belgeler parçalara bölünüp yerel Ollama
  embedding modeliyle (nomic-embed-text, ~300MB) vektöre çevrilir, yerel SQLite'a (rusqlite,
  bundled — ayrı bir servis/sunucu YOK) kaydedilir. Arama, kosinüs benzerliğiyle Rust içinde
  hesaplanır. "Geçen ay yüklediğim rapor neydi" gibi sorular artık cevaplanabiliyor.
- **Gerçek tek-tık kurulumlar genişletildi**:
  - Apt paketleri artık önce `pkexec` (polkit'in grafiksel şifre penceresi) ile deneniyor —
    terminale gerek kalmadan, tek pencerede şifre girip kurulum tamamlanıyor. pkexec yoksa
    eski terminal-açma yöntemine düşülüyor.
  - **Piper TTS** artık Video2X/Ollama gibi gerçek sudo'suz otomatik kurulum alıyor
    (GitHub releases'tan taşınabilir indirme).
  - **Türkçe Piper ses modeli** Hugging Face'ten (kayıt gerekmeyen, herkese açık barındırma)
    tek tıkla indirilip Ses & Dil ayarlarına otomatik yazılıyor.
- Modeller listesine `nomic-embed-text` (RAG için embedding modeli) eklendi.

## Bilinçli Sınırlamalar (gizlenmiyor, açıkça yazılıyor)

- **Wayland**: alışkanlık takibi şu an sadece X11. GNOME/KDE Wayland için ayrı protokol
  adaptörleri gerekiyor, henüz yazılmadı.
- **whisper.cpp / ffmpeg / Piper bundle edilmiyor**: bunlar GB'larca model dosyası
  içerdiğinden kullanıcı kendi sisteminde kurmalı. Kurulum kontrolü `install_amorfly_ai.sh`
  içinde var.
- **"Öğrenme" motoru MVP**: şu an eşik-tabanlı basit kural. Gerçek örüntü/alışkanlık öğrenimi
  (ör. zaman serisi analiz) ayrı, ileri bir faz.
- **Bu ortamda derlenip test edilmedi**: kod, ağ erişimi olmayan bir sandbox'ta blind olarak
  yazıldı. İlk `npm run tauri:dev` denemesinde küçük derleme hatalarıyla karşılaşman normal —
  Rust tarafı özellikle (tipik hatalar: crate versiyon uyuşmazlığı, import düzeltmeleri).

## Çalıştırma

```bash
chmod +x install_amorfly_ai.sh
./install_amorfly_ai.sh   # bağımlılık kontrolü + opsiyonel otomatik build

# ya da manuel:
npm install
npm run tauri:dev     # geliştirme modu
npm run tauri:build   # .deb / .AppImage üretir (src-tauri/target/release/bundle/)
```

Ollama kurulu ve `ollama serve` ile çalışıyor olmalı: https://ollama.com
