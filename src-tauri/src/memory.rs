// Amorfly AI — Ortak Hafıza Katmanı
//
// Bu, sohbet geçmişinden ayrı bir şey: AI'nin "gerçek çalışma hafızası".
// Belge analizi, altyazı üretimi, video kalite artırma, toplu görevler
// gibi işlemler burada otomatik birikir. Sohbet başladığında kısa bir
// özet (digest) modele context olarak veriliyor — böylece model, daha
// önce hangi dosyalarla ne yaptığını "hatırlıyormuş" gibi davranabiliyor.
//
// Şifreli vault üzerinde saklanır (security.rs). Kullanıcı Ayarlar'dan
// hafızayı görüntüleyip tamamen temizleyebilir — "kayıtsız/bağımsız"
// felsefesiyle tutarlı: hafıza da tamamen yerel ve kullanıcının kontrolünde.

use crate::security::{vault_read, vault_write};
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct MemoryEntry {
    pub timestamp: String,
    pub category: String, // "tercih" | "islem" | "aliskanlik-ozet"
    pub text: String,
}

const MEMORY_KEY: &str = "memory";
const MAX_ENTRIES: usize = 300;

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Rust içinden (başka modüllerden) çağrılabilen dahili fonksiyon.
/// Hata olursa sessizce loglar, hafıza yazımı asla ana işlemi bozmaz.
pub fn remember(category: String, text: String) {
    let result: Result<(), String> = (|| {
        let mut entries: Vec<MemoryEntry> = match vault_read(MEMORY_KEY.to_string())? {
            Some(json) => serde_json::from_str(&json).unwrap_or_default(),
            None => vec![],
        };
        entries.push(MemoryEntry { timestamp: now_str(), category, text });
        if entries.len() > MAX_ENTRIES {
            let excess = entries.len() - MAX_ENTRIES;
            entries.drain(0..excess);
        }
        vault_write(MEMORY_KEY.to_string(), serde_json::to_string(&entries).map_err(|e| e.to_string())?)
    })();

    if let Err(e) = result {
        crate::logger::log_line("ERROR", &format!("Hafızaya yazılamadı: {}", e));
    }
}

/// Frontend'den bir tercihi/notu bilerek hafızaya eklemek için.
#[tauri::command]
pub fn remember_preference(text: String) {
    remember("tercih".to_string(), text);
}

#[tauri::command]
pub fn recall_all() -> Result<Vec<MemoryEntry>, String> {
    match vault_read(MEMORY_KEY.to_string())? {
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub fn clear_memory() -> Result<(), String> {
    let _ = vault_write(SUMMARY_KEY.to_string(), "".to_string());
    vault_write(MEMORY_KEY.to_string(), "[]".to_string())
}

/// Sohbete eklenmek üzere kısa, okunabilir bir hafıza özeti üretir.
#[tauri::command]
pub fn memory_digest(max_entries: usize) -> Result<String, String> {
    let all = recall_all()?;
    if all.is_empty() {
        return Ok(String::new());
    }
    let recent: Vec<&MemoryEntry> = all.iter().rev().take(max_entries).collect();
    let mut out = String::from("Kullanıcı ve geçmiş işlemler hakkında bildiklerin:\n");
    for e in recent.iter().rev() {
        out.push_str(&format!("- [{}] {}: {}\n", e.timestamp, e.category, e.text));
    }
    Ok(out)
}

#[derive(Serialize, Deserialize)]
struct CachedSummary {
    text: String,
    entry_count_at_summary: usize,
}

const SUMMARY_KEY: &str = "memory_summary_cache";
/// Bu kadar yeni kayıt birikmeden özet yeniden üretilmez — her mesajda
/// Ollama'ya gereksiz istek atılmasını önler.
const RESUMMARIZE_THRESHOLD: usize = 12;

/// Ham kayıtları olduğu gibi sıralamak yerine, gerçekten Ollama'ya
/// özetleterek "kullanıcı son zamanlarda ne üzerinde çalışıyor" tarzında
/// anlamlı, kompakt bir bağlam üretir. Sık çağrılsa bile (her sohbet
/// mesajında) pahalı yeniden özetleme sadece yeterince yeni kayıt
/// birikince tetiklenir — arada önbellekten döner.
#[tauri::command]
pub async fn summarize_memory(model: String) -> Result<String, String> {
    let all = recall_all()?;
    if all.len() < 5 {
        // Çok az veri var, özetlemeye değmez — ham haliyle dön.
        return memory_digest(50);
    }

    let cached: Option<CachedSummary> = vault_read(SUMMARY_KEY.to_string())?
        .and_then(|json| serde_json::from_str(&json).ok());

    if let Some(c) = &cached {
        if all.len() < c.entry_count_at_summary + RESUMMARIZE_THRESHOLD {
            return Ok(c.text.clone());
        }
    }

    let raw_log: String = all
        .iter()
        .rev()
        .take(80)
        .rev()
        .map(|e| format!("[{}] {}: {}", e.timestamp, e.category, e.text))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Aşağıda bir kullanıcının masaüstü AI asistanıyla geçmiş işlem kayıtları var. \
         Bunu 3-4 cümlelik, akıcı bir Türkçe özete dönüştür: kullanıcının hangi konularda \
         çalıştığını, hangi tür dosyalarla uğraştığını ve varsa tekrar eden tercihlerini vurgula. \
         Sadece özeti yaz, başka açıklama ekleme.\n\n--- KAYITLAR ---\n{}",
        raw_log
    );

    let client = crate::http_client_with_timeout(120);
    let num_ctx = ((prompt.len() / 3) as u32 + 512).clamp(2048, 131072);
    let body = serde_json::json!({
        "model": if model.is_empty() { "llama3.2".to_string() } else { model },
        "messages": [{ "role": "user", "content": prompt }],
        "options": { "num_ctx": num_ctx },
        "stream": false,
    });

    let res = client
        .post("http://127.0.0.1:11434/api/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama'ya ulaşılamadı: {}", e))?;

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let summary = json["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Özet üretilemedi.".to_string())?;

    let cache = CachedSummary { text: summary.clone(), entry_count_at_summary: all.len() };
    vault_write(SUMMARY_KEY.to_string(), serde_json::to_string(&cache).map_err(|e| e.to_string())?)?;

    Ok(summary)
}
