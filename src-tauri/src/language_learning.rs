// Amorfly AI — Dil Eğitimi Oturum Saklama
//
// Gerçek sohbet çağrısı (Ollama'ya istek) burada YOK — bunun için zaten
// var olan ollama_chat komutu kullanılıyor (frontend'den). Bu modül
// sadece "kaldığın yerden devam et" özelliğini sağlıyor: her dil/seviye/
// senaryo kombinasyonu için ayrı bir oturum, tüm mesaj geçmişiyle birlikte
// şifreli vault'ta saklanıyor — memory.rs ile aynı güvenlik prensibi.

use crate::security::{vault_read, vault_write};
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LanguageMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LanguageSession {
    pub id: String,
    pub dil: String,
    pub seviye: String,
    pub senaryo: String,
    pub mesajlar: Vec<LanguageMessage>,
    pub son_guncelleme: String,
}

const KEY: &str = "language_learning_sessions";

fn load_all() -> Result<Vec<LanguageSession>, String> {
    match vault_read(KEY.to_string())? {
        Some(json) => serde_json::from_str(&json).map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

fn save_all(sessions: &[LanguageSession]) -> Result<(), String> {
    let json = serde_json::to_string(sessions).map_err(|e| e.to_string())?;
    vault_write(KEY.to_string(), json)
}

#[tauri::command]
pub fn list_language_sessions() -> Result<Vec<LanguageSession>, String> {
    let mut all = load_all()?;
    // En son güncellenen en üstte — kaldığı yerden devam etmesi kolay olsun.
    all.sort_by(|a, b| b.son_guncelleme.cmp(&a.son_guncelleme));
    Ok(all)
}

#[tauri::command]
pub fn create_language_session(dil: String, seviye: String, senaryo: String) -> Result<LanguageSession, String> {
    let session = LanguageSession {
        id: uuid::Uuid::new_v4().to_string(),
        dil,
        seviye,
        senaryo,
        mesajlar: Vec::new(),
        son_guncelleme: Local::now().to_rfc3339(),
    };
    let mut all = load_all()?;
    all.push(session.clone());
    save_all(&all)?;
    Ok(session)
}

/// Bir oturumun TÜM mesaj listesini üzerine yazar (frontend her yeni
/// mesajdan sonra güncel listeyi gönderir — basit ve tutarlı).
#[tauri::command]
pub fn save_language_session(id: String, mesajlar: Vec<LanguageMessage>) -> Result<(), String> {
    let mut all = load_all()?;
    if let Some(s) = all.iter_mut().find(|s| s.id == id) {
        s.mesajlar = mesajlar;
        s.son_guncelleme = Local::now().to_rfc3339();
    } else {
        return Err("Oturum bulunamadı.".to_string());
    }
    save_all(&all)
}

/// Seviye tespit sınavı bitince (frontend LEVEL_RESULT_MARKER'ı algılayınca)
/// oturumun seviye/senaryo alanlarını günceller — "Seviye Belirleniyor"
/// durumundan gerçek bir CEFR seviyesine ve önerilen ders programına geçiş.
#[tauri::command]
pub fn update_language_session_meta(id: String, seviye: String, senaryo: String) -> Result<(), String> {
    let mut all = load_all()?;
    if let Some(s) = all.iter_mut().find(|s| s.id == id) {
        s.seviye = seviye;
        s.senaryo = senaryo;
        s.son_guncelleme = Local::now().to_rfc3339();
    } else {
        return Err("Oturum bulunamadı.".to_string());
    }
    save_all(&all)
}

#[tauri::command]
pub fn delete_language_session(id: String) -> Result<(), String> {
    let mut all = load_all()?;
    all.retain(|s| s.id != id);
    save_all(&all)
}

/// Seviye tespit testi bitince, oturumun seviyesini kalıcı olarak
/// günceller (başlangıçta "Seviye Belirleniyor" olan alan gerçek bir
/// seviyeye (A1-C1) dönüşür).
#[tauri::command]
pub fn update_language_session_level(id: String, seviye: String) -> Result<(), String> {
    let mut all = load_all()?;
    if let Some(s) = all.iter_mut().find(|s| s.id == id) {
        s.seviye = seviye;
        s.son_guncelleme = Local::now().to_rfc3339();
    } else {
        return Err("Oturum bulunamadı.".to_string());
    }
    save_all(&all)
}
