// Amorfly AI — Ses ve Dil Kalitesi Modülü
//
// - record_and_transcribe: mikrofonu ffmpeg (PulseAudio/PipeWire-pulse
//   uyumluluk katmanı) ile sabit bir süre kaydeder, whisper.cpp ile
//   metne çevirir. Basit ve durum (state) yönetimi gerektirmeyen bir
//   tasarım: "N saniye kaydet" — start/stop toggle değil.
// - speak_text: Piper TTS ile metni sese çevirip ffplay ile çalar.
// - refine_language: modelin ürettiği metni ikinci bir geçişle hedef
//   dilde dilbilgisi kurallarına uygun, akıcı hale getirir — devrik/saçma
//   cümleleri önlemek için. Anlam değiştirmeden SADECE üslubu düzeltir.

use serde::Serialize;
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

fn tmp_path(suffix: &str) -> String {
    format!("/tmp/amorfly_{}{}", Uuid::new_v4(), suffix)
}

/// Mikrofonu `duration_secs` saniye kaydedip whisper.cpp ile metne çevirir.
#[tauri::command]
pub async fn record_and_transcribe(
    duration_secs: u32,
    whisper_bin: String,
    whisper_model_path: String,
    language: String,
) -> Result<String, String> {
    let wav_path = tmp_path(".wav");

    let status = Command::new("ffmpeg")
        .args([
            "-y", "-f", "pulse", "-i", "default",
            "-ar", "16000", "-ac", "1", "-t", &duration_secs.to_string(),
            &wav_path,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!(
            "Mikrofon kaydı başlatılamadı (ffmpeg + PulseAudio/PipeWire gerekir). Detay: {}", e
        ))?;

    if !status.success() {
        return Err("Mikrofon kaydı başarısız oldu. Sistem ses ayarlarını kontrol edin.".to_string());
    }

    let json_out = format!("{}.json", wav_path);
    let status = Command::new(&whisper_bin)
        .args(["-m", &whisper_model_path, "-f", &wav_path, "-oj", "-of", &wav_path, "-l", &language])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("whisper.cpp çalıştırılamadı: {}", e))?;

    let _ = tokio::fs::remove_file(&wav_path).await;

    if !status.success() {
        return Err("Ses tanıma başarısız oldu.".to_string());
    }

    let raw = tokio::fs::read_to_string(&json_out)
        .await
        .map_err(|e| format!("Tanıma sonucu okunamadı: {}", e))?;
    let _ = tokio::fs::remove_file(&json_out).await;

    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let mut text = String::new();
    if let Some(arr) = parsed["transcription"].as_array() {
        for seg in arr {
            if let Some(t) = seg["text"].as_str() {
                text.push_str(t);
            }
        }
    }

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err("Konuşma algılanamadı — mikrofona daha yakın/açık konuşmayı deneyin.".to_string());
    }
    Ok(trimmed)
}

#[derive(Serialize)]
pub struct SpeechResult {
    pub wav_path: String,
}

/// Metni Piper TTS ile seslendirir ve arka planda çalar (bekletmeden döner).
#[tauri::command]
pub async fn speak_text(text: String, piper_bin: String, voice_model: String) -> Result<SpeechResult, String> {
    let wav_path = tmp_path(".wav");

    let mut child = Command::new(&piper_bin)
        .args(["--model", &voice_model, "--output_file", &wav_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("piper bulunamadı: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(text.as_bytes()).await;
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Seslendirme (piper) başarısız oldu.".to_string());
    }

    // Çalmayı bekletmeden başlat — UI donmasın.
    let _ = Command::new("ffplay")
        .args(["-nodisp", "-autoexit", "-loglevel", "quiet", &wav_path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    Ok(SpeechResult { wav_path })
}

/// Modelin ürettiği metni ikinci bir geçişle hedef dilde dilbilgisi
/// kurallarına uygun, akıcı hale getirir. Anlamı değiştirmez, sadece
/// devrik/saçma cümle kuruluşunu düzeltir.
#[tauri::command]
pub async fn refine_language(text: String, model: String, target_language: String) -> Result<String, String> {
    let prompt = format!(
        "Aşağıdaki metni {} dilinde, dilbilgisi kurallarına tamamen uygun, akıcı ve doğal \
         cümlelerle YENİDEN YAZ. Anlamı ASLA değiştirme, hiçbir bilgi ekleme veya çıkarma — \
         sadece cümle kuruluşunu ve dilbilgisini düzelt. SADECE düzeltilmiş metni döndür, \
         başka hiçbir açıklama, önsöz ya da not ekleme.\n\nMetin:\n{}",
        target_language, text
    );

    let client = crate::http_client_with_timeout(60);
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
        .map_err(|e| format!("Ollama'ya ulaşılamadı: {}", e))?;

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    json["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "Dil düzeltmesi başarısız oldu.".to_string())
}
