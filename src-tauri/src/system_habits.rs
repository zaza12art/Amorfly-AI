// Amorfly AI — Kullanım Alışkanlığı Takibi (Faz 4, MVP)
//
// AÇIK SINIRLAMA: Bu modül şu an yalnızca X11 destekler (`xdotool`
// üzerinden). Wayland'de aktif pencere takibi masaüstü ortamına göre
// değişen bir protokol (wlr-foreign-toplevel, ya da GNOME/KDE'ye özel
// portallar) gerektirir ve henüz yazılmadı. GNOME Wayland ve KDE Wayland
// için ayrı adaptörler ileride eklenecek.
//
// Gizlilik ilkesi: hiçbir pencere başlığı/kullanım verisi ağa gönderilmez.
// Sadece yerelde, şifrelenmiş vault içinde (security.rs) tutulur.

use crate::security::{vault_read, vault_write};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct HabitLog {
    // uygulama/pencere adı -> toplam saniye
    pub totals_seconds: HashMap<String, u64>,
    pub last_active_app: String,
    pub last_seen_unix: u64,
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// `xdotool` ile aktif pencere başlığını okur. X11 dışında (Wayland) ya
/// da xdotool kurulu değilse net bir hata döner — sessizce sahte veri
/// üretmez.
fn active_window_x11() -> Result<String, String> {
    let output = Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
        .map_err(|_| "xdotool bulunamadı (X11 gerekli). Wayland'de bu özellik henüz desteklenmiyor.".to_string())?;

    if !output.status.success() {
        return Err("Aktif pencere okunamadı (ör. hiçbir pencere odakta değil).".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Tek seferlik bir "tik": aktif pencereyi okur, geçen süreyi önceki
/// çağrıdan bu yana o pencerenin toplamına ekler. Frontend bunu periyodik
/// (ör. her 30 saniyede bir) çağırmalı.
#[tauri::command]
pub fn record_activity_tick(interval_seconds: u64) -> Result<HabitLog, String> {
    let window = active_window_x11()?;

    let mut log: HabitLog = match vault_read("habits".to_string())? {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => HabitLog::default(),
    };

    *log.totals_seconds.entry(window.clone()).or_insert(0) += interval_seconds;
    log.last_active_app = window;
    log.last_seen_unix = now_unix();

    let json = serde_json::to_string(&log).map_err(|e| e.to_string())?;
    vault_write("habits".to_string(), json)?;

    Ok(log)
}

#[tauri::command]
pub fn get_habit_log() -> Result<HabitLog, String> {
    match vault_read("habits".to_string())? {
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        None => Ok(HabitLog::default()),
    }
}

/// Basit, kural-tabanlı öneri motoru (MVP). Gerçek "öğrenme" (kalıp
/// tespiti/ML) ileride bu fonksiyonun yerini alacak; şimdilik dürüst bir
/// eşik-tabanlı mantık var, "yapay zeka öğrendi" gibi abartılı bir iddia
/// yapmıyoruz.
#[tauri::command]
pub fn suggest_from_habits() -> Result<Option<String>, String> {
    let log = get_habit_log()?;
    if let Some((_, secs)) = log.totals_seconds.iter().max_by_key(|(_, v)| **v) {
        if *secs > 2 * 3600 {
            return Ok(Some(format!(
                "'{}' üzerinde uzun süredir çalışıyorsunuz ({} dakika). Kısa bir mola vermek ister misiniz?",
                log.last_active_app,
                secs / 60
            )));
        }
    }
    Ok(None)
}
