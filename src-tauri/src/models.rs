// Amorfly AI — Gerçek Model İndirme Yöneticisi
// Eski `store.ts` bir mock veritabanıydı (sahte ilerleme yüzdeleri).
// Bu modül Ollama'nın kendi HTTP /api/pull uç noktasını (gerçek,
// satır-satır garanti edilmiş JSON akışı) kullanarak indirir.
//
// ÖNEMLİ NOT (önceki sürümden farkı): İlk sürüm `ollama pull` komutunu
// bir alt-süreç (subprocess) olarak çalıştırıp çıktısını satır satır
// okuyordu. Bu iki gerçek soruna yol açtı:
//   1) CLI'nin ilerleme çubuğu muhtemelen "\r" (satırı üstüne yazma)
//      kullanıyor, biz "\n" (gerçek yeni satır) beklediğimiz için her
//      katman tam bitene kadar HİÇ ilerleme gelmiyordu — kullanıcıya
//      "takılmış" gibi görünüyordu.
//   2) Masaüstü uygulaması olarak (.desktop/AppImage üzerinden) başlatılan
//      süreçlerin PATH'i, bir terminalden farklı olabilir — "ollama"
//      komutu elle kurulmuş olsa bile bulunamayabilirdi.
// HTTP API'ye geçmek ikisini de çözüyor: hem gerçekten "\n" ile ayrılmış
// JSON satırları garantili, hem de subprocess/PATH bağımlılığı hiç yok
// (tıpkı sohbet isteklerimiz gibi doğrudan 127.0.0.1:11434'e konuşuyoruz).

use futures_util::StreamExt;
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
pub struct ModelProgress {
    pub model: String,
    pub status: String,
    pub percent: f32,
    pub done: bool,
    pub error: Option<String>,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    /// Saniyedeki indirme hızı (byte/sn) — internet hızına göre canlı ölçülür.
    pub speed_bytes_per_sec: f64,
    /// Mevcut katman/dosya için tahmini kalan süre (saniye). Hız henüz
    /// ölçülemediyse (ilk an) None.
    pub eta_seconds: Option<u64>,
}

/// Bilinen, önerilen model listesi. Bu bir "mağaza" değil, sadece
/// kullanıcıya başlangıç için öneri — hepsi gerçekten `ollama pull` ile
/// indirilir, sahte kayıt yok.
#[derive(Serialize)]
pub struct SuggestedModel {
    pub id: String,
    pub label: String,
    pub approx_size_gb: f32,
    pub note: String,
}

#[tauri::command]
pub fn suggested_models() -> Vec<SuggestedModel> {
    vec![
        SuggestedModel {
            id: "llama3.2".into(),
            label: "Llama 3.2 (3B) — genel amaçlı, hafif".into(),
            approx_size_gb: 2.0,
            note: "Çoğu sistemde CPU ile bile akıcı çalışır.".into(),
        },
        SuggestedModel {
            id: "qwen2.5:7b".into(),
            label: "Qwen 2.5 (7B) — güçlü akıl yürütme".into(),
            approx_size_gb: 4.7,
            note: "8GB+ RAM önerilir.".into(),
        },
        SuggestedModel {
            id: "qwen2.5-coder:7b".into(),
            label: "Qwen 2.5 Coder (7B) — kod üretimi (AutoLISP, Python, Bash, SQL, VBA...)".into(),
            approx_size_gb: 4.7,
            note: "Niş diller/DSL'lerde genel amaçlı modellerden çok daha tutarlı sonuç verir.".into(),
        },
        SuggestedModel {
            id: "llava:7b".into(),
            label: "LLaVA (7B) — görsel analiz (çok-modlu)".into(),
            approx_size_gb: 4.7,
            note: "Resim/ekran görüntüsü yükleyip yorumlatmak için gerekli.".into(),
        },
        SuggestedModel {
            id: "nomic-embed-text".into(),
            label: "Nomic Embed Text — belge arama (RAG) için gerekli".into(),
            approx_size_gb: 0.3,
            note: "Çok küçük ve hızlı. Belge & Görsel sekmesinde 'aranabilir hafıza' özelliği için gerekli.".into(),
        },
        SuggestedModel {
            id: "gemma2:2b".into(),
            label: "Gemma 2 (2B) — çok hafif sistemler".into(),
            approx_size_gb: 1.6,
            note: "Zayıf donanımlar için.".into(),
        },
    ]
}

/// Ollama'nın /api/pull uç noktasına HTTP isteği atar ve akıştaki HER
/// gerçek JSON satırını (Ollama sunucu tarafında \n ile garantili ayrılmış)
/// pencereye "amorfly://model-progress" olayı olarak yayınlar. Sahte
/// yüzde/hız ÜRETMEZ — yüzdeyi Ollama'nın "completed"/"total" alanlarından,
/// indirme hızını ise ardışık iki olay arasındaki gerçek byte/zaman
/// farkından canlı olarak hesaplar (bu yüzden internet hızına göre değişir).
#[tauri::command]
pub async fn pull_model(app: AppHandle, model: String) -> Result<(), String> {
    let client = crate::http_client_with_timeout(3600);
    let res = client
        .post("http://127.0.0.1:11434/api/pull")
        .json(&serde_json::json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Ollama'ya ulaşılamadı (çalıştığından emin ol): {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let err = format!("Ollama hata döndürdü ({}): {}", status, text);
        let _ = app.emit(
            "amorfly://model-progress",
            ModelProgress {
                model: model.clone(), status: "hata".into(), percent: 0.0, done: true, error: Some(err.clone()),
                completed_bytes: 0, total_bytes: 0, speed_bytes_per_sec: 0.0, eta_seconds: None,
            },
        );
        return Err(err);
    }

    let mut stream = res.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();

    // Hız ölçümü: aynı katman (digest) için ardışık iki olay arasındaki
    // byte/zaman farkı. Katman değişince (yeni dosya inmeye başlayınca)
    // sıfırlanır — böylece bir önceki dosyanın hızı yeni dosyaya sızmaz.
    let mut current_digest: Option<String> = None;
    let mut window_start = Instant::now();
    let mut window_start_bytes: u64 = 0;
    let mut last_speed: f64 = 0.0;
    // Son gönderilen olay üzerinden en az ~250ms geçmeden yeni olay
    // yayınlama — çok sık event basmak arayüzü gereksiz yere yorar.
    let mut last_emit = Instant::now() - std::time::Duration::from_secs(1);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buf.extend_from_slice(&chunk);

        // Ollama'nın akışı gerçekten \n ile ayrılmış JSON nesneleridir
        // (ndjson) — CLI'nin \r tabanlı ilerleme çubuğunun aksine bu
        // format satır satır güvenilir şekilde bölünebilir.
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line_bytes);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let json: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue, // bozuk/yarım satır — atla, sonraki chunk'ta tamamlanabilir
            };

            if let Some(err_msg) = json.get("error").and_then(|e| e.as_str()) {
                let _ = app.emit(
                    "amorfly://model-progress",
                    ModelProgress {
                        model: model.clone(), status: "hata".into(), percent: 0.0, done: true, error: Some(err_msg.to_string()),
                        completed_bytes: 0, total_bytes: 0, speed_bytes_per_sec: 0.0, eta_seconds: None,
                    },
                );
                return Err(err_msg.to_string());
            }

            let status_text = json.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let digest = json.get("digest").and_then(|d| d.as_str()).map(|s| s.to_string());
            let total = json.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
            let completed = json.get("completed").and_then(|c| c.as_u64()).unwrap_or(0);
            let percent = if total > 0 { (completed as f32 / total as f32) * 100.0 } else { 0.0 };
            let is_done = status_text == "success";

            // Katman değiştiyse ölçüm penceresini sıfırla.
            if digest != current_digest {
                current_digest = digest.clone();
                window_start = Instant::now();
                window_start_bytes = completed;
                last_speed = 0.0;
            }

            let elapsed = window_start.elapsed().as_secs_f64();
            if elapsed >= 0.3 && completed > window_start_bytes {
                let bytes_delta = (completed - window_start_bytes) as f64;
                last_speed = bytes_delta / elapsed;
                window_start = Instant::now();
                window_start_bytes = completed;
            }

            let eta_seconds = if last_speed > 1024.0 && total > completed {
                Some(((total - completed) as f64 / last_speed) as u64)
            } else {
                None
            };

            // Throttle: saniyede en fazla ~4 olay yayınla (görsel akıcılık
            // için yeterli, gereksiz IPC trafiğine girmeden).
            let should_emit = is_done || last_emit.elapsed().as_millis() >= 250;
            if should_emit {
                last_emit = Instant::now();
                let _ = app.emit(
                    "amorfly://model-progress",
                    ModelProgress {
                        model: model.clone(),
                        status: status_text,
                        percent,
                        done: is_done,
                        error: None,
                        completed_bytes: completed,
                        total_bytes: total,
                        speed_bytes_per_sec: last_speed,
                        eta_seconds,
                    },
                );
            }
        }
    }

    let _ = app.emit(
        "amorfly://model-progress",
        ModelProgress {
            model: model.clone(), status: "tamamlandı".into(), percent: 100.0, done: true, error: None,
            completed_bytes: 0, total_bytes: 0, speed_bytes_per_sec: 0.0, eta_seconds: None,
        },
    );

    Ok(())
}
