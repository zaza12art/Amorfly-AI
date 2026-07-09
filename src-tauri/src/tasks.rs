// Amorfly AI — Görev Motoru (Task Engine)
//
// Basit ama gerçek bir arka plan görev kuyruğu. Üç somut görev türü:
//   1. folder_scan       — klasörü tarar, dosya/tür/boyut raporu çıkarır
//   2. batch_ocr_pdfs    — klasördeki tüm PDF'lere tesseract-ocr uygular
//   3. batch_upscale     — klasördeki tüm videoları Video2X ile büyütür
//
// İZİN KATMANI: Toplu/kalıcı-etkili görevler (`confirmed: bool` parametresi
// olmadan) ÇALIŞTIRILMAZ. Frontend, kullanıcıya önce bir onay penceresi
// gösterip yalnızca "evet" derse confirmed=true göndermeli. Rust tarafı da
// bunu ayrıca kontrol eder — tek katmana güvenilmez.
//
// Tüm araçlar açık kaynak ve ücretsiz: tesseract-ocr, poppler-utils
// (pdftoppm), video2x. Hiçbir ücretli/kayıt gerektiren servis yok.

use crate::logger::log_line;
use crate::memory;
use serde::{Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::process::Command;
use uuid::Uuid;

#[derive(Serialize, Clone)]
pub struct Task {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String, // kuyrukta | çalışıyor | tamamlandı | hata | iptal edildi
    pub progress: f32,
    pub log: Vec<String>,
    pub result: Option<String>,
}

pub type TaskStore = Arc<Mutex<HashMap<String, Task>>>;

fn new_task(kind: &str, title: &str) -> Task {
    Task {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_string(),
        title: title.to_string(),
        status: "kuyrukta".to_string(),
        progress: 0.0,
        log: vec![],
        result: None,
    }
}

fn update<F: FnOnce(&mut Task)>(store: &TaskStore, id: &str, f: F) {
    if let Ok(mut map) = store.lock() {
        if let Some(t) = map.get_mut(id) {
            f(t);
        }
    }
}

fn is_cancelled(store: &TaskStore, id: &str) -> bool {
    store.lock().map(|m| m.get(id).map(|t| t.status == "iptal edildi").unwrap_or(false)).unwrap_or(false)
}

fn emit(app: &AppHandle, store: &TaskStore) {
    if let Ok(map) = store.lock() {
        let list: Vec<Task> = map.values().cloned().collect();
        let _ = app.emit("amorfly://tasks-updated", list);
    }
}

// --- agent.rs (iş akışı planlayıcısı) tarafından yeniden kullanılan
// paylaşılan yardımcılar. Aynı görev kuyruğuna (TaskStore) yazdıkları için
// iş akışları da "Görevler" sekmesinde diğer görevlerle birlikte görünür.
pub fn insert_manual_task(store: &TaskStore, id: &str, kind: &str, title: &str) {
    let mut t = new_task(kind, title);
    t.id = id.to_string();
    if let Ok(mut map) = store.lock() {
        map.insert(id.to_string(), t);
    }
}
pub fn set_status(store: &TaskStore, id: &str, status: &str) {
    update(store, id, |t| t.status = status.to_string());
}
pub fn set_progress(store: &TaskStore, id: &str, p: f32) {
    update(store, id, |t| t.progress = p);
}
pub fn push_log(store: &TaskStore, id: &str, line: &str) {
    update(store, id, |t| t.log.push(line.to_string()));
}
pub fn fail_task(store: &TaskStore, id: &str, err: &str) {
    update(store, id, |t| {
        t.status = "hata".to_string();
        t.result = Some(err.to_string());
    });
}
pub fn complete_task(store: &TaskStore, id: &str, result: &str) {
    update(store, id, |t| {
        t.status = "tamamlandı".to_string();
        t.progress = 100.0;
        t.result = Some(result.to_string());
    });
}
pub fn cancelled(store: &TaskStore, id: &str) -> bool {
    is_cancelled(store, id)
}
pub fn emit_public(app: &AppHandle, store: &TaskStore) {
    emit(app, store);
}

#[tauri::command]
pub fn list_tasks(store: State<'_, TaskStore>) -> Vec<Task> {
    store.lock().map(|m| m.values().cloned().collect()).unwrap_or_default()
}

#[tauri::command]
pub fn cancel_task(store: State<'_, TaskStore>, app: AppHandle, id: String) {
    update(&store, &id, |t| t.status = "iptal edildi".to_string());
    emit(&app, &store);
}

fn walk_dir(path: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk_dir(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn ext_lower(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

// ---------------------------------------------------------------------
// 1) KLASÖR TARAMA — salt okunur, onay gerektirmez
// ---------------------------------------------------------------------
#[tauri::command]
pub async fn queue_folder_scan(app: AppHandle, store: State<'_, TaskStore>, folder_path: String) -> Result<String, String> {
    let task = new_task("folder_scan", &format!("Klasör taraması: {}", folder_path));
    let id = task.id.clone();
    store.lock().map_err(|_| "kilit hatası")?.insert(id.clone(), task);
    emit(&app, &store);

    let store2 = store.inner().clone();
    let app2 = app.clone();
    let id2 = id.clone();

    tokio::spawn(async move {
        update(&store2, &id2, |t| t.status = "çalışıyor".to_string());
        emit(&app2, &store2);
        log_line("INFO", &format!("Klasör taraması başladı: {}", folder_path));

        let mut files = Vec::new();
        walk_dir(Path::new(&folder_path), &mut files);

        let mut by_ext: HashMap<String, (u64, u64)> = HashMap::new(); // (adet, byte)
        let mut total_bytes: u64 = 0;
        for f in &files {
            let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
            total_bytes += size;
            let e = by_ext.entry(ext_lower(f)).or_insert((0, 0));
            e.0 += 1;
            e.1 += size;
        }

        let mut sorted: Vec<(&String, &(u64, u64))> = by_ext.iter().collect();
        sorted.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));

        let mut report = format!(
            "Toplam {} dosya, {:.2} GB.\n\nTür dağılımı (büyükten küçüğe):\n",
            files.len(),
            total_bytes as f64 / 1_073_741_824.0
        );
        for (ext, (count, bytes)) in sorted.iter().take(15) {
            report.push_str(&format!(
                "  .{:<8} {:>5} dosya   {:>8.2} MB\n",
                if ext.is_empty() { "?" } else { ext },
                count,
                *bytes as f64 / 1_048_576.0
            ));
        }

        update(&store2, &id2, |t| {
            t.status = "tamamlandı".to_string();
            t.progress = 100.0;
            t.result = Some(report.clone());
        });
        log_line("INFO", &format!("Klasör taraması bitti: {} ({} dosya)", folder_path, files.len()));
        emit(&app2, &store2);
    });

    Ok(id)
}

// ---------------------------------------------------------------------
// 2) TOPLU OCR (PDF klasörü) — tesseract-ocr + pdftoppm, ONAY GEREKİR
// ---------------------------------------------------------------------
#[tauri::command]
pub async fn queue_batch_ocr(
    app: AppHandle,
    store: State<'_, TaskStore>,
    folder_path: String,
    confirmed: bool,
) -> Result<String, String> {
    if !confirmed {
        return Err("Onaylanmamış toplu işlem — güvenlik nedeniyle çalıştırılmadı.".to_string());
    }

    let task = new_task("batch_ocr", &format!("Toplu OCR: {}", folder_path));
    let id = task.id.clone();
    store.lock().map_err(|_| "kilit hatası")?.insert(id.clone(), task);
    emit(&app, &store);

    let store2 = store.inner().clone();
    let app2 = app.clone();
    let id2 = id.clone();

    tokio::spawn(async move {
        update(&store2, &id2, |t| t.status = "çalışıyor".to_string());
        emit(&app2, &store2);
        log_line("INFO", &format!("Toplu OCR başladı: {}", folder_path));

        let mut all_files = Vec::new();
        walk_dir(Path::new(&folder_path), &mut all_files);
        let pdfs: Vec<PathBuf> = all_files.into_iter().filter(|p| ext_lower(p) == "pdf").collect();

        let total = pdfs.len();
        let mut done = 0usize;
        let mut failed = 0usize;

        for pdf in &pdfs {
            if is_cancelled(&store2, &id2) {
                break;
            }

            match ocr_single_pdf(pdf).await {
                Ok(out_path) => {
                    done += 1;
                    update(&store2, &id2, |t| t.log.push(format!("✓ {}", out_path)));
                }
                Err(e) => {
                    failed += 1;
                    update(&store2, &id2, |t| t.log.push(format!("✗ {}: {}", pdf.display(), e)));
                    log_line("ERROR", &format!("OCR hatası ({}): {}", pdf.display(), e));
                }
            }

            let progress = if total > 0 { ((done + failed) as f32 / total as f32) * 100.0 } else { 100.0 };
            update(&store2, &id2, |t| t.progress = progress);
            emit(&app2, &store2);
        }

        let summary = format!("{} PDF işlendi, {} başarılı, {} hatalı.", total, done, failed);
        update(&store2, &id2, |t| {
            t.status = if is_cancelled(&store2, &id2) { "iptal edildi".to_string() } else { "tamamlandı".to_string() };
            t.progress = 100.0;
            t.result = Some(summary.clone());
        });

        memory::remember("islem".to_string(), format!("'{}' klasöründe toplu OCR yaptı — {}", folder_path, summary));
        log_line("INFO", &format!("Toplu OCR bitti: {}", summary));
        emit(&app2, &store2);
    });

    Ok(id)
}

async fn ocr_single_pdf(pdf_path: &Path) -> Result<String, String> {
    let tmp_dir = std::env::temp_dir().join(format!("amorfly_ocr_{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&tmp_dir).await.map_err(|e| e.to_string())?;
    let prefix = tmp_dir.join("page");

    let status = Command::new("pdftoppm")
        .args(["-r", "200", "-png", pdf_path.to_str().unwrap_or(""), prefix.to_str().unwrap_or("")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("pdftoppm bulunamadı: {}", e))?;
    if !status.success() {
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        return Err("PDF sayfaları görsele çevrilemedi.".to_string());
    }

    let mut pages: Vec<PathBuf> = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&tmp_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            pages.push(entry.path());
        }
    }
    pages.sort();

    let mut full_text = String::new();
    for page in &pages {
        let output = Command::new("tesseract")
            .args([page.to_str().unwrap_or(""), "stdout", "-l", "tur+eng"])
            .output()
            .await
            .map_err(|e| format!("tesseract bulunamadı: {}", e))?;
        full_text.push_str(&String::from_utf8_lossy(&output.stdout));
        full_text.push_str("\n\n");
    }

    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;

    let out_path = format!("{}.ocr.txt", pdf_path.display());
    tokio::fs::write(&out_path, full_text).await.map_err(|e| format!("Sonuç yazılamadı: {}", e))?;
    Ok(out_path)
}

// ---------------------------------------------------------------------
// 3) TOPLU VİDEO KALİTE ARTIRMA — Video2X, ONAY GEREKİR
// ---------------------------------------------------------------------
#[tauri::command]
pub async fn queue_batch_upscale(
    app: AppHandle,
    store: State<'_, TaskStore>,
    folder_path: String,
    scale: u32,
    model: String,
    confirmed: bool,
) -> Result<String, String> {
    if !confirmed {
        return Err("Onaylanmamış toplu işlem — güvenlik nedeniyle çalıştırılmadı.".to_string());
    }

    let task = new_task("batch_upscale", &format!("Toplu video kalite artırma: {}", folder_path));
    let id = task.id.clone();
    store.lock().map_err(|_| "kilit hatası")?.insert(id.clone(), task);
    emit(&app, &store);

    let store2 = store.inner().clone();
    let app2 = app.clone();
    let id2 = id.clone();

    tokio::spawn(async move {
        update(&store2, &id2, |t| t.status = "çalışıyor".to_string());
        emit(&app2, &store2);

        let mut all_files = Vec::new();
        walk_dir(Path::new(&folder_path), &mut all_files);
        let videos: Vec<PathBuf> = all_files
            .into_iter()
            .filter(|p| matches!(ext_lower(p).as_str(), "mp4" | "mkv" | "webm" | "avi" | "mov"))
            .collect();

        let total = videos.len();
        let mut done = 0usize;
        let mut failed = 0usize;

        for video in &videos {
            if is_cancelled(&store2, &id2) {
                break;
            }

            let video_str = video.to_string_lossy().to_string();
            match crate::upscale::upscale_video(app2.clone(), video_str.clone(), scale, model.clone(), None).await {
                Ok(res) => {
                    done += 1;
                    update(&store2, &id2, |t| t.log.push(format!("✓ {}", res.output_path)));
                }
                Err(e) => {
                    failed += 1;
                    update(&store2, &id2, |t| t.log.push(format!("✗ {}: {}", video.display(), e)));
                }
            }

            let progress = if total > 0 { ((done + failed) as f32 / total as f32) * 100.0 } else { 100.0 };
            update(&store2, &id2, |t| t.progress = progress);
            emit(&app2, &store2);
        }

        let summary = format!("{} video işlendi, {} başarılı, {} hatalı.", total, done, failed);
        update(&store2, &id2, |t| {
            t.status = if is_cancelled(&store2, &id2) { "iptal edildi".to_string() } else { "tamamlandı".to_string() };
            t.progress = 100.0;
            t.result = Some(summary.clone());
        });

        memory::remember("islem".to_string(), format!("'{}' klasöründe toplu video kalite artırma yaptı — {}", folder_path, summary));
        log_line("INFO", &format!("Toplu video kalite artırma bitti: {}", summary));
        emit(&app2, &store2);
    });

    Ok(id)
}
