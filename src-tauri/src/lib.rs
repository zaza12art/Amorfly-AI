mod security;
mod models;
mod online;
mod system_habits;
mod subtitles;
mod upscale;
mod installer;
mod voice;
mod documents;
mod logger;
mod diagnostics;
mod tasks;
mod memory;
mod rag;
mod agent;
mod queue;
mod excel_gen;
mod reference_data;
mod language_learning;
mod smart_chat;
mod router;

use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

const OLLAMA_HOST: &str = "http://127.0.0.1:11434";

/// GERÇEK GÜVENLİK KONTROLÜ: `reqwest::Client::new()` (proje genelinde 17
/// yerde) HİÇBİR zaman aşımı içermiyordu — Ollama/LibreTranslate/online
/// sağlayıcı tıkanır ya da aşırı yavaşlarsa istek SONSUZA KADAR bekliyordu.
/// Kuyruk mekanizmamız (queue.rs) bu TEK isteği bitene kadar TÜM ağır
/// işlemleri kilitliyordu — gerçek, kanıtlanabilir bir "donma" kaynağı.
/// Artık her istek türüne göre makul bir üst sınır var; süre dolunca istek
/// düzgün bir hata verir, uygulama asla sonsuza kadar askıda kalmaz.
pub fn http_client_with_timeout(secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(secs))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct GpuInfo {
    pub vendor: String,
    pub model: String,
    pub total_vram_mb: u64,
    pub free_vram_mb: u64,
    pub source: String,
}

#[tauri::command]
async fn check_ollama() -> bool {
    http_client_with_timeout(5)
        .get(OLLAMA_HOST)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

pub async fn call_ollama(model: String, messages: Vec<ChatMessage>) -> Result<String, String> {
    let _guard = queue::acquire().await;
    // Uzun cevap üretimi için makul ama sonsuz olmayan bir üst sınır.
    let client = http_client_with_timeout(300);
    let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    // Önceki sürümde her mesaj için EN AZ 8192'lik bağlam penceresi
    // zorlanıyordu — kısa bir "merhaba" bile gereksiz yere büyük bir
    // KV-cache ayırıyor, özellikle CPU'da her yanıtı fark edilir şekilde
    // yavaşlatıyordu. Artık taban değer Ollama'nın kendi varsayılanına
    // yakın (2048), sadece gerçekten uzun girdilerde büyütülüyor.
    let num_ctx = ((total_chars / 3) as u32 + 512).clamp(2048, 131072);
    let body = serde_json::json!({
        "model": if model.is_empty() { "llama3.2".to_string() } else { model },
        "messages": messages,
        "options": { "num_ctx": num_ctx },
        "stream": false,
    });

    let res = client
        .post(format!("{}/api/chat", OLLAMA_HOST))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!(
            "Ollama'ya ulaşılamadı. Terminalde 'ollama serve' çalıştığından emin olun. Detay: {}",
            e
        ))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Ollama hata döndürdü ({}): {}", status, text));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| format!("Ollama yanıtı çözümlenemedi: {}", e))?;
    json["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Ollama yanıtında beklenen içerik bulunamadı.".to_string())
}

#[tauri::command]
async fn ollama_chat(model: String, messages: Vec<ChatMessage>) -> Result<String, String> {
    call_ollama(model, messages).await
}

#[tauri::command]
async fn list_ollama_models() -> Result<Vec<String>, String> {
    let client = http_client_with_timeout(15);
    let res = client
        .get(format!("{}/api/tags", OLLAMA_HOST))
        .send()
        .await
        .map_err(|e| format!("Ollama'ya ulaşılamadı: {}", e))?;
    let json: serde_json::Value = res.json().await.map_err(|e| format!("Model listesi çözümlenemedi: {}", e))?;
    Ok(json["models"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
        .unwrap_or_default())
}

#[tauri::command]
fn get_gpu_info() -> GpuInfo {
    if let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total,memory.free", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = text.trim().split(',').map(|s| s.trim()).collect();
            if parts.len() == 3 {
                return GpuInfo {
                    vendor: "NVIDIA".to_string(),
                    model: parts[0].to_string(),
                    total_vram_mb: parts[1].parse().unwrap_or(0),
                    free_vram_mb: parts[2].parse().unwrap_or(0),
                    source: "nvidia-smi".to_string(),
                };
            }
        }
    }

    if let Ok(output) = Command::new("lspci").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = text.lines().find(|l| l.to_lowercase().contains("vga")) {
            return GpuInfo {
                vendor: "Generic".to_string(),
                model: line.to_string(),
                total_vram_mb: 0,
                free_vram_mb: 0,
                source: "lspci (VRAM tespiti yok, sürücüye bağlı)".to_string(),
            };
        }
    }

    GpuInfo {
        vendor: "Bilinmiyor".to_string(),
        model: "Tespit edilemedi".to_string(),
        total_vram_mb: 0,
        free_vram_mb: 0,
        source: "none".to_string(),
    }
}

/// Faz 4: öneri balonunu gerçek bir masaüstü bildirimi olarak gösterir.
#[tauri::command]
fn show_suggestion_notification(app: tauri::AppHandle, message: String) -> Result<(), String> {
    app.notification()
        .builder()
        .title("Amorfly AI")
        .body(message)
        .show()
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logger::init_logging();

    tauri::Builder::default()
        .manage(tasks::TaskStore::default())
        .manage(rag::RagDb(std::sync::Mutex::new(rag::init_db())))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Sistem tepsisi (tray) simgesi — öneri balonları ve hızlı erişim için
            let show = MenuItem::with_id(app, "show", "Amorfly AI'ı Göster", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_ollama,
            ollama_chat,
            list_ollama_models,
            get_gpu_info,
            show_suggestion_notification,
            security::encrypt_string,
            security::decrypt_string,
            security::vault_write,
            security::vault_read,
            models::suggested_models,
            models::pull_model,
            online::save_online_provider,
            online::get_online_provider,
            online::clear_online_provider,
            online::online_chat,
            system_habits::record_activity_tick,
            system_habits::get_habit_log,
            system_habits::suggest_from_habits,
            subtitles::generate_turkish_subtitles,
            subtitles::generate_turkish_dub,
            upscale::list_upscale_gpus,
            upscale::upscale_video,
            upscale::interpolate_framerate,
            installer::install_ollama_portable,
            installer::install_video2x_portable,
            installer::install_piper_portable,
            installer::download_piper_turkish_voice,
            voice::record_and_transcribe,
            voice::speak_text,
            voice::refine_language,
            documents::extract_document_text,
            documents::analyze_document,
            documents::analyze_image,
            documents::export_document,
            excel_gen::export_steel_profiles_excel,
            excel_gen::export_rebar_table_excel,
            excel_gen::generate_excel_from_description,
            queue::queue_status,
            language_learning::list_language_sessions,
            language_learning::create_language_session,
            language_learning::save_language_session,
            language_learning::update_language_session_meta,
            language_learning::delete_language_session,
            language_learning::update_language_session_level,
            smart_chat::smart_chat,
            router::get_router_config,
            router::save_router_config,
            router::suggest_model_for_task,
            logger::log_frontend_error,
            logger::get_recent_logs,
            logger::get_log_file_path,
            diagnostics::get_app_version,
            diagnostics::run_diagnostics,
            diagnostics::open_terminal_install,
            tasks::list_tasks,
            tasks::cancel_task,
            tasks::queue_folder_scan,
            tasks::queue_batch_ocr,
            tasks::queue_batch_upscale,
            memory::remember_preference,
            memory::recall_all,
            memory::clear_memory,
            memory::memory_digest,
            memory::summarize_memory,
            rag::index_document,
            rag::search_documents,
            rag::clear_document_index,
            rag::indexed_document_count,
            agent::plan_workflow,
            agent::run_workflow,
        ])
        .run(tauri::generate_context!())
        .expect("Amorfly AI çalıştırılırken hata oluştu");
}
