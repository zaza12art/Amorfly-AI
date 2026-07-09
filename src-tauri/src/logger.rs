// Amorfly AI — Log Sistemi ve Panik Yakalayıcı
//
// - Her oturumda logs/ altında tarih-saat damgalı bir dosya açılır.
// - Her satır kendi zaman damgasını taşır.
// - std::panic::set_hook ile: bir modülde panik olursa uygulama
//   ÇÖKMEDEN (Cargo.toml'da panic="abort" KULLANILMADIĞI için) hatayı
//   log dosyasına yazar. Kullanıcı "çalışmıyor" dediğinde bu dosyaya
//   bakarak nedenini görebilirsin.
// - Frontend'deki her `catch` bloğu da `log_frontend_error` ile aynı
//   dosyaya yazabilir — böylece tek bir yerde tüm hatalar birikir.

use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static LOG_FILE: OnceLock<Mutex<PathBuf>> = OnceLock::new();

fn logs_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Amorfly AI");
    dir.push("logs");
    dir
}

/// Uygulama başlarken bir kez çağrılır.
pub fn init_logging() {
    let dir = logs_dir();
    let _ = fs::create_dir_all(&dir);

    let filename = format!("amorfly_{}.log", Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let path = dir.join(filename);
    let _ = LOG_FILE.set(Mutex::new(path));

    // Panik yakalayıcı: bir modülde panik olursa uygulamayı çökertmeden
    // logla. Cargo.toml'daki panic="unwind" (varsayılan) ile birlikte
    // çalışır — abort değil.
    std::panic::set_hook(Box::new(|info| {
        log_line("PANIC", &format!("{}", info));
    }));

    log_line("INFO", &format!("Amorfly AI v{} başlatıldı", env!("CARGO_PKG_VERSION")));
}

pub fn log_line(level: &str, message: &str) {
    if let Some(lock) = LOG_FILE.get() {
        if let Ok(path) = lock.lock() {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&*path) {
                let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
                let _ = writeln!(file, "[{}] [{}] {}", ts, level, message);
            }
        }
    }
    eprintln!("[{}] {}", level, message);
}

/// Frontend'deki hata yakalama (catch) bloklarından çağrılır — tek bir
/// yerde toplanan hatalar, kullanıcı "çalışmıyor" dediğinde teşhisi
/// kolaylaştırır.
#[tauri::command]
pub fn log_frontend_error(message: String) {
    log_line("FRONTEND_ERROR", &message);
}

#[tauri::command]
pub fn get_recent_logs(lines: usize) -> Vec<String> {
    if let Some(lock) = LOG_FILE.get() {
        if let Ok(path) = lock.lock() {
            if let Ok(content) = fs::read_to_string(&*path) {
                let all: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let start = all.len().saturating_sub(lines);
                return all[start..].to_vec();
            }
        }
    }
    vec![]
}

#[tauri::command]
pub fn get_log_file_path() -> String {
    LOG_FILE
        .get()
        .and_then(|l| l.lock().ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}
