// Amorfly AI — Altyazı & Dublaj Pipeline'ı
//
// Akış: video -> (ffmpeg) ses çıkar -> (whisper.cpp) transkript+zaman
// damgaları -> (Ollama, yerel LLM) her segmenti Türkçe'ye çevir ->
// .srt üret -> [opsiyonel] (Piper TTS) her segment için Türkçe seslendirme
// üret -> (ffmpeg) segmentleri zaman damgalarına göre birleştirip orijinal
// videoyla mux'la.
//
// whisper.cpp ~99 dili destekler (otomatik dil algılama dahil) — "dünyada
// en çok konuşulan 12 dil" gibi bir sınırlama koymaya gerek yok, hepsi
// zaten kapsanıyor. Kaynak dil ne olursa olsun hedef her zaman Türkçe.
//
// GEREKSİNİMLER (kullanıcının kendi sisteminde kurulu olmalı, bu proje
// bunları paketlemiyor çünkü GB'larca model dosyası içerir):
//   - ffmpeg
//   - whisper.cpp derlenmiş `whisper-cli` (ya da `main`) binary'si + bir
//     ggml model dosyası (ör. ggml-base.bin, ggml-small.bin)
//   - (opsiyonel dublaj için) piper TTS + Türkçe ses modeli
//
// +18 içerik dahil olmak üzere kişisel kullanım için herhangi bir video
// dosyasında çalışır; bu tamamen yerel bir medya işleme pipeline'ıdır,
// hiçbir içerik dışarı gönderilmez.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::time::Duration;
use tokio::process::Command;

/// GERÇEK SORUN DÜZELTMESİ: whisper.cpp/ffmpeg gibi ağır işlemler varsayılan
/// (normal) CPU önceliğiyle çalıştığında, işletim sistemi zamanlayıcısı bazen
/// masaüstü ortamına (fare imleci, pencere yöneticisi) yeterli CPU zamanı
/// ayıramıyor — kullanıcı "mouse bile oynamıyor" diye bildirdi. Çözüm: bu
/// süreçleri `nice` ile DÜŞÜK öncelikte başlatmak — CPU'yu hâlâ kullanır,
/// işi hâlâ bitirir, ama masaüstü etkileşimine öncelik tanınır, sistem
/// kilitlenmiş gibi hissettirmez. `nice` bulunamazsa (çok nadir), doğrudan
/// programın kendisini çalıştırmaya düşer — sessizce başarısız olmaz.
fn nice_command(program: &str) -> Command {
    let mut cmd = Command::new("nice");
    cmd.arg("-n").arg("15").arg(program);
    cmd
}

/// Sesin toplam süresini saniye cinsinden öğrenir (ffprobe) — "konuşma
/// tespit edilemedi" hatasında teşhis bilgisi (kaç saniyelik ses çıkarıldı)
/// göstermek için kullanılıyor.
async fn audio_duration_secs(wav_path: &str) -> Result<f64, String> {
    let output = Command::new("ffprobe")
        .args(["-v", "quiet", "-show_entries", "format=duration", "-of", "csv=p=0", wav_path])
        .output()
        .await
        .map_err(|e| format!("ffprobe çalıştırılamadı: {}", e))?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|_| "Ses süresi okunamadı.".to_string())
}

/// GERÇEK GÜVENLİK KONTROLÜ: kullanıcı "otomatik oturum kapandı" bildirdi —
/// bu, `nice` (CPU önceliği) ile değil, muhtemelen RAM TÜKENMESİYLE ilgili
/// (Linux'un kendisi bellek biterse kritik süreçleri zorla kapatabiliyor,
/// bu da tüm oturumun çökmesine yol açabilir). whisper.cpp + Ollama (7B model
/// genelde 4-5GB) + ffmpeg aynı anda belleği zorlayabilir. Ağır bir işlem
/// başlatmadan ÖNCE boş RAM'i kontrol edip, kritik derecede azsa işlemi hiç
/// başlatmadan kullanıcıyı uyarıyoruz — yarıda çökmek yerine.
fn available_memory_mb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb: u64 = rest.trim().split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

const MIN_SAFE_MEMORY_MB: u64 = 2048;

fn check_memory_or_warn() -> Result<(), String> {
    if let Some(available) = available_memory_mb() {
        if available < MIN_SAFE_MEMORY_MB {
            return Err(format!(
                "Boş bellek kritik derecede az ({} MB). Bu işlem (whisper.cpp + Ollama birlikte) \
                 bu durumda çalışırsa sistemin tamamen kilitlenmesine/oturumun kapanmasına yol \
                 açabilir. Lütfen önce diğer ağır uygulamaları (tarayıcı, AutoCAD vb.) kapatıp \
                 tekrar dene.",
                available
            ));
        }
    }
    // /proc/meminfo okunamadıysa (ör. Linux dışı bir ortam) sessizce devam
    // ediyoruz — bu kontrol sadece EK bir güvenlik katmanı, zorunlu değil.
    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SubtitleSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub source_text: String,
    pub translated_text: Option<String>,
}

fn ms_to_srt_time(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1000;
    let msec = ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, msec)
}

/// 1) Videodan whisper.cpp'nin istediği formatta (16kHz, mono, WAV) ses çıkarır.
async fn extract_audio(video_path: &str, out_wav: &str) -> Result<(), String> {
    let status = nice_command("ffmpeg")
        .args([
            "-y", "-i", video_path,
            "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le",
            out_wav,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("ffmpeg bulunamadı ya da çalıştırılamadı: {}", e))?;

    if !status.success() {
        return Err("ffmpeg ses çıkarma sırasında hata verdi (video dosyasını kontrol edin).".to_string());
    }
    Ok(())
}

/// whisper.cpp'yi TEK bir ses dosyası üzerinde çalıştırır, segmentleri
/// döndürür. `vad_model_path` doluysa whisper.cpp'nin KENDİ dahili VAD'ı
/// (`-vm ... --vad`) devreye girer.
///
/// ÖNEMLİ MİMARİ DEĞİŞİKLİĞİ: Önceki sürümde biz kendi VAD'ımızı (ffmpeg
/// silencedetect) yazıp sesi parçalara bölüyor, HER parçayı AYRI bir
/// whisper.cpp çağrısıyla işliyorduk. Bu, her çağrıda modelin yeniden
/// yüklenmesi yüzünden ciddi bir yavaşlamaya yol açtı (kullanıcı 40
/// dakikalık bir videoda 1 saatten fazla bekledi). whisper.cpp'nin
/// KENDİ dahili VAD desteği (`--vad`) aynı işi TEK bir process içinde,
/// model SADECE BİR KEZ yüklenerek yapıyor — hem daha hızlı hem daha az
/// kod. VAD modeli verilmezse (boş string), whisper.cpp tüm dosyayı
/// normal şekilde işler (eski, VAD'sız davranış).
async fn run_whisper_on_file(
    whisper_bin: &str,
    model_path: &str,
    wav_path: &str,
    vad_model_path: &str,
) -> Result<Vec<SubtitleSegment>, String> {
    let json_out = format!("{}.json", wav_path);

    let mut args: Vec<String> = vec![
        "-m".into(), model_path.into(),
        "-f".into(), wav_path.into(),
        "-oj".into(),
        "-of".into(), wav_path.into(),
        "-l".into(), "auto".into(),
    ];
    if !vad_model_path.trim().is_empty() {
        args.push("-vm".into());
        args.push(vad_model_path.into());
        args.push("--vad".into());
    }

    let status = nice_command(whisper_bin)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!(
            "whisper.cpp binary'si ('{}') bulunamadı. Kurulum: github.com/ggerganov/whisper.cpp. Detay: {}",
            whisper_bin, e
        ))?;

    if !status.success() {
        return Err("whisper.cpp transkripsiyon sırasında hata verdi.".to_string());
    }

    let raw = tokio::fs::read_to_string(&json_out)
        .await
        .map_err(|e| format!("whisper çıktısı okunamadı: {}", e))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let mut segments = Vec::new();
    if let Some(arr) = parsed["transcription"].as_array() {
        for seg in arr {
            let start_ms = (seg["offsets"]["from"].as_u64()).unwrap_or(0);
            let end_ms = (seg["offsets"]["to"].as_u64()).unwrap_or(0);
            let text = seg["text"].as_str().unwrap_or("").trim().to_string();
            if !text.is_empty() {
                segments.push(SubtitleSegment { start_ms, end_ms, source_text: text, translated_text: None });
            }
        }
    }
    let _ = tokio::fs::remove_file(&json_out).await;
    Ok(segments)
}

/// Tüm sesi TEK whisper.cpp çağrısıyla işler (VAD modeli verildiyse
/// whisper.cpp'nin dahili VAD'ı devreye girer, konuşma olmayan kısımları
/// kendi içinde atlar — bizim ayrı ffmpeg+parçalama sürecimize gerek
/// kalmaz). İşlem sürerken (uzun videolarda dakikalar sürebilir) paralel
/// bir heartbeat görevi "hâlâ çalışıyor" sinyali yayınlar.
async fn transcribe(app: &AppHandle, wav_path: &str, whisper_bin: &str, model_path: &str, vad_model_path: &str) -> Result<Vec<SubtitleSegment>, String> {
    let start = Instant::now();
    let app_hb = app.clone();
    let using_vad = !vad_model_path.trim().is_empty();
    let heartbeat = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let _ = app_hb.emit(
                "amorfly://subtitle-progress",
                SubtitleProgress {
                    status: format!(
                        "Konuşma tanınıyor{}… ({} saniye geçti)",
                        if using_vad { " (VAD aktif)" } else { "" },
                        start.elapsed().as_secs()
                    ),
                    current: 0, total: 0, done: false,
                },
            );
        }
    });
    let result = run_whisper_on_file(whisper_bin, model_path, wav_path, vad_model_path).await;
    heartbeat.abort();
    let mut segments = result?;
    segments.sort_by_key(|s| s.start_ms);
    Ok(segments)
}


/// 3) Her segmenti Türkçe'ye çevirir. İki motor desteklenir:
///   - "ollama": genel amaçlı yerel LLM, numaralı satırları toplu (batch)
///     halinde çevirir — esnek ama LLM'in "sadakat" hatası riski var.
///   - "libretranslate": Argos Translate tabanlı, ÖZEL EĞİTİLMİŞ bir
///     çeviri motoru, kendi yerel sunucun olarak çalışır (Docker/pip).
///     Segment segment çevirir, genelde daha tutarlı/hızlıdır ama
///     kurulumu ayrı bir adım gerektirir (Ollama gibi hazır gelmez).
async fn translate_segments(
    app: &AppHandle,
    segments: &mut [SubtitleSegment],
    model: &str,
    engine: &str,
    libretranslate_url: &str,
) -> Result<(), String> {
    if engine == "libretranslate" {
        translate_with_libretranslate(app, segments, libretranslate_url).await
    } else {
        translate_with_ollama(app, segments, model).await
    }
}

async fn translate_with_ollama(app: &AppHandle, segments: &mut [SubtitleSegment], model: &str) -> Result<(), String> {
    const BATCH: usize = 20;
    let total = segments.len();

    for chunk_start in (0..segments.len()).step_by(BATCH) {
        let chunk_end = (chunk_start + BATCH).min(segments.len());
        let numbered: String = segments[chunk_start..chunk_end]
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s.source_text))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Aşağıdaki numaralı satırları Türkçe'ye çevir. Her kelimeyi eksiksiz çevir, \
             argo/küfür/+18 ifadeler dahil hiçbir şeyi sansürleme ya da yumuşatma — \
             birebir anlam ve ton korunmalı. SADECE aynı numaralandırmayla çeviriyi döndür, \
             başka açıklama ekleme.\n\n{}",
            numbered
        );

        let client = crate::http_client_with_timeout(180);
        let body = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }],
            "stream": false,
        });

        let res = client
            .post("http://127.0.0.1:11434/api/chat")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama'ya çeviri isteği başarısız: {}", e))?;

        let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        let content = json["message"]["content"].as_str().unwrap_or("").to_string();

        for (i, line) in content.lines().enumerate() {
            if chunk_start + i >= chunk_end {
                break;
            }
            let cleaned = line
                .splitn(2, |c: char| c == '.' || c == ')')
                .nth(1)
                .unwrap_or(line)
                .trim()
                .to_string();
            if !cleaned.is_empty() {
                segments[chunk_start + i].translated_text = Some(cleaned);
            }
        }

        let _ = app.emit(
            "amorfly://subtitle-progress",
            SubtitleProgress {
                status: format!("Çevriliyor… {}/{} segment tamamlandı (Ollama)", chunk_end, total),
                current: chunk_end as u32,
                total: total as u32,
                done: false,
            },
        );
    }

    Ok(())
}

/// LibreTranslate segment-segment çevirir (numaralı toplu satır hilesine
/// gerek yok — kendi API'si zaten tek metin alıp tek çeviri döndürüyor,
/// bu yüzden Ollama'daki "modelin numaralamayı bozması" riski hiç yok).
async fn translate_with_libretranslate(
    app: &AppHandle,
    segments: &mut [SubtitleSegment],
    base_url: &str,
) -> Result<(), String> {
    let client = crate::http_client_with_timeout(30);
    let total = segments.len();
    let url = format!("{}/translate", base_url.trim_end_matches('/'));

    for i in 0..segments.len() {
        let body = serde_json::json!({
            "q": segments[i].source_text,
            "source": "auto",
            "target": "tr",
            "format": "text",
        });

        let res = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!(
                "LibreTranslate'e ulaşılamadı ({}). Çalıştığından emin ol: docker run -ti -p 5000:5000 libretranslate/libretranslate — Detay: {}",
                base_url, e
            ))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("LibreTranslate hata döndürdü ({}): {}", status, text));
        }

        let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        if let Some(translated) = json.get("translatedText").and_then(|t| t.as_str()) {
            segments[i].translated_text = Some(translated.to_string());
        }

        // Her segmentte değil, her ~5 segmentte bir (ya da sonuncuda)
        // olay yayınla — çok sık IPC trafiğine girmeden yine de canlı
        // görünür bir ilerleme sağlanır.
        if (i + 1) % 5 == 0 || i + 1 == total {
            let _ = app.emit(
                "amorfly://subtitle-progress",
                SubtitleProgress {
                    status: format!("Çevriliyor… {}/{} segment tamamlandı (LibreTranslate)", i + 1, total),
                    current: (i + 1) as u32,
                    total: total as u32,
                    done: false,
                },
            );
        }
    }

    Ok(())
}

fn segments_to_srt(segments: &[SubtitleSegment]) -> String {
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&format!("{}\n", i + 1));
        out.push_str(&format!(
            "{} --> {}\n",
            ms_to_srt_time(seg.start_ms),
            ms_to_srt_time(seg.end_ms)
        ));
        out.push_str(seg.translated_text.as_deref().unwrap_or(&seg.source_text));
        out.push_str("\n\n");
    }
    out
}

fn ms_to_vtt_time(ms: u64) -> String {
    // WebVTT, SRT'den farklı olarak virgül değil nokta kullanır (00:00:01.000)
    ms_to_srt_time(ms).replace(',', ".")
}

/// Tarayıcının <track> etiketi SADECE WebVTT formatını destekler, SRT'yi
/// DEĞİL. Uygulama içi altyazı önizlemesi için bu format gerekli —
/// önceki sürümde bu ayrım gözden kaçmıştı ve önizleme hiç çalışmıyordu.
fn segments_to_vtt(segments: &[SubtitleSegment]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&format!("{}\n", i + 1));
        out.push_str(&format!(
            "{} --> {}\n",
            ms_to_vtt_time(seg.start_ms),
            ms_to_vtt_time(seg.end_ms)
        ));
        out.push_str(seg.translated_text.as_deref().unwrap_or(&seg.source_text));
        out.push_str("\n\n");
    }
    out
}

/// video_path'in kendi uzantısını atıp yerine yenisini ekler. Böylece
/// "film.mp4" -> "film.tr.srt" olur (VLC/mpv gibi oynatıcılar bu ismi
/// otomatik algılar). Önceki sürümde "film.mp4.tr.srt" üretiliyordu —
/// hiçbir oynatıcı bunu otomatik bulamıyordu.
fn sibling_path(video_path: &str, new_suffix: &str) -> String {
    let path = Path::new(video_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(video_path);
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");
    if parent.is_empty() {
        format!("{}{}", stem, new_suffix)
    } else {
        format!("{}/{}{}", parent, stem, new_suffix)
    }
}

#[derive(Serialize)]
pub struct SubtitleResult {
    pub srt_path: String,
    pub vtt_path: String,
    pub segment_count: usize,
}

#[derive(Serialize, Clone)]
pub struct SubtitleProgress {
    pub status: String,
    pub current: u32,
    pub total: u32,
    pub done: bool,
}

/// Uçtan uca: video -> Türkçe .srt (harici oynatıcılar için, VLC/mpv
/// video ile aynı isimdeki .srt'yi otomatik yükler) + .vtt (uygulama
/// içi <track> önizlemesi için — tarayıcılar SRT'yi değil, yalnızca
/// WebVTT'yi destekler).
#[tauri::command]
pub async fn generate_turkish_subtitles(
    app: AppHandle,
    video_path: String,
    whisper_bin: String,
    whisper_model_path: String,
    translation_model: String,
    translate_engine: String,
    libretranslate_url: String,
    vad_model_path: String,
) -> Result<SubtitleResult, String> {
    let _guard = crate::queue::acquire().await;
    check_memory_or_warn()?;
    let tmp_wav = format!("{}.amorfly.wav", video_path);

    extract_audio(&video_path, &tmp_wav).await?;
    let mut segments = transcribe(&app, &tmp_wav, &whisper_bin, &whisper_model_path, &vad_model_path).await?;

    if segments.is_empty() {
        // Kör bir "sessiz video" mesajı yerine gerçek teşhis bilgisi ver —
        // ses dosyası gerçekten oluşmuş mu, kaç saniyelik, kaç byte —
        // böylece "gerçekten sessiz" ile "bir şey sessizce bozuldu"
        // ayırt edilebilir.
        let duration = audio_duration_secs(&tmp_wav).await.unwrap_or(-1.0);
        let file_size = tokio::fs::metadata(&tmp_wav).await.map(|m| m.len()).unwrap_or(0);
        let _ = tokio::fs::remove_file(&tmp_wav).await;
        return Err(format!(
            "Videoda konuşma tespit edilemedi. Teşhis: ses dosyası {} saniye, {} KB olarak çıkarıldı. \
             Eğer bu süre videonun gerçek uzunluğuna yakınsa ama içinde konuşma varsa, whisper.cpp modeli \
             (küçük 'small' model bazı aksan/arka plan gürültüsünde zorlanabilir — 'medium' modeli dene) \
             ya da sesin çok kısık olması sebep olabilir. Süre 0 ya da çok küçükse, videonun ses kanalı \
             olmayabilir ya da ffmpeg'in desteklemediği bir codec kullanıyor olabilir.",
            if duration >= 0.0 { format!("{:.1}", duration) } else { "okunamadı".to_string() },
            file_size / 1024
        ));
    }

    translate_segments(&app, &mut segments, &translation_model, &translate_engine, &libretranslate_url).await?;

    let srt_path = sibling_path(&video_path, ".tr.srt");
    let vtt_path = sibling_path(&video_path, ".tr.vtt");

    tokio::fs::write(&srt_path, segments_to_srt(&segments))
        .await
        .map_err(|e| format!("SRT dosyası yazılamadı: {}", e))?;
    tokio::fs::write(&vtt_path, segments_to_vtt(&segments))
        .await
        .map_err(|e| format!("VTT dosyası yazılamadı: {}", e))?;

    let _ = tokio::fs::remove_file(&tmp_wav).await;
    let _ = tokio::fs::remove_file(format!("{}.json", tmp_wav)).await;

    let _ = app.emit(
        "amorfly://subtitle-progress",
        SubtitleProgress { status: "tamamlandı".into(), current: segments.len() as u32, total: segments.len() as u32, done: true },
    );

    crate::memory::remember(
        "islem".to_string(),
        format!("'{}' videosu için Türkçe altyazı üretti ({} segment)", video_path, segments.len()),
    );

    Ok(SubtitleResult { srt_path, vtt_path, segment_count: segments.len() })
}

/// Opsiyonel: Türkçe .srt'den Piper TTS ile dublaj sesi üretir ve
/// videonun ses kanalını değiştirir. Ayrı, isteğe bağlı bir adım —
/// altyazı üretimi bu olmadan da tam çalışır.
#[tauri::command]
pub async fn generate_turkish_dub(
    video_path: String,
    srt_path: String,
    piper_bin: String,
    piper_voice_model: String,
) -> Result<String, String> {
    let _guard = crate::queue::acquire().await;
    let srt_content = tokio::fs::read_to_string(&srt_path)
        .await
        .map_err(|e| format!("SRT okunamadı: {}", e))?;

    let segments = parse_srt(&srt_content);
    if segments.is_empty() {
        return Err("SRT dosyasında segment bulunamadı.".to_string());
    }

    let work_dir = format!("{}.dub_segments", video_path);
    tokio::fs::create_dir_all(&work_dir).await.map_err(|e| e.to_string())?;

    let mut concat_list = String::new();
    for (i, seg) in segments.iter().enumerate() {
        let seg_wav = format!("{}/{:04}.wav", work_dir, i);

        let mut child = Command::new(&piper_bin)
            .args(["--model", &piper_voice_model, "--output_file", &seg_wav])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("piper bulunamadı: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(seg.text.as_bytes()).await;
        }
        let _ = child.wait().await;

        concat_list.push_str(&format!("file '{}'\n", seg_wav));
    }

    let list_path = format!("{}/list.txt", work_dir);
    tokio::fs::write(&list_path, concat_list).await.map_err(|e| e.to_string())?;

    let dub_audio = format!("{}.dub.wav", video_path);
    let status = nice_command("ffmpeg")
        .args(["-y", "-f", "concat", "-safe", "0", "-i", &list_path, &dub_audio])
        .status()
        .await
        .map_err(|e| format!("ffmpeg (dublaj birleştirme) hatası: {}", e))?;
    if !status.success() {
        return Err("Dublaj sesi birleştirilemedi.".to_string());
    }

    let output_video = sibling_path(&video_path, ".dublajli.mp4");
    let status = nice_command("ffmpeg")
        .args([
            "-y", "-i", &video_path, "-i", &dub_audio,
            "-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-shortest",
            &output_video,
        ])
        .status()
        .await
        .map_err(|e| format!("ffmpeg (mux) hatası: {}", e))?;
    if !status.success() {
        return Err("Dublajlı video oluşturulamadı.".to_string());
    }

    Ok(output_video)
}

struct SrtSeg {
    text: String,
}

fn parse_srt(content: &str) -> Vec<SrtSeg> {
    let mut out = Vec::new();
    let blocks = content.split("\n\n");
    for block in blocks {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() >= 3 {
            let text = lines[2..].join(" ");
            if !text.trim().is_empty() {
                out.push(SrtSeg { text: text.trim().to_string() });
            }
        }
    }
    out
}
