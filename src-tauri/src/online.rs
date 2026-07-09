// Amorfly AI — Online Sağlayıcı Köprüsü (Opsiyonel)
// Hiçbir sağlayıcı hardcoded değildir. Kullanıcı, ayarlardan kendi
// tercih ettiği OpenAI-uyumlu bir endpoint (OpenRouter, Groq, kendi
// sunucusu, vb.) ve API anahtarını girer. Anahtar `security.rs`
// üzerinden şifrelenmiş olarak diskte tutulur. Bu tamamen opsiyoneldir;
// hiçbir şey buraya otomatik/varsayılan olarak bağlanmaz.

use crate::security::{vault_read, vault_write};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct OnlineProviderConfig {
    pub label: String,     // ör. "OpenRouter (ücretsiz katman)"
    pub base_url: String,  // ör. "https://openrouter.ai/api/v1"
    pub api_key: String,
    pub model: String,
}

#[derive(Serialize, Deserialize)]
struct ChatMsg {
    role: String,
    content: String,
}

#[tauri::command]
pub fn save_online_provider(config: OnlineProviderConfig) -> Result<(), String> {
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    vault_write("online_provider".to_string(), json)
}

#[tauri::command]
pub fn get_online_provider() -> Result<Option<OnlineProviderConfig>, String> {
    match vault_read("online_provider".to_string())? {
        Some(json) => {
            let cfg: OnlineProviderConfig = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            Ok(Some(cfg))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn clear_online_provider() -> Result<(), String> {
    vault_write("online_provider".to_string(), "".to_string())
}

pub async fn call_online(prompt: String, history: Vec<ChatMsgArg>) -> Result<String, String> {
    let _guard = crate::queue::acquire().await;
    let cfg = get_online_provider()?
        .ok_or("Kayıtlı bir online sağlayıcı yok. Ayarlardan ekleyin veya yerel modda kalın.")?;

    let client = crate::http_client_with_timeout(120);
    let mut messages: Vec<ChatMsg> = history
        .into_iter()
        .map(|m| ChatMsg { role: m.role, content: m.content })
        .collect();
    messages.push(ChatMsg { role: "user".to_string(), content: prompt });

    let body = serde_json::json!({
        "model": cfg.model,
        "messages": messages,
    });

    let res = client
        .post(format!("{}/chat/completions", cfg.base_url.trim_end_matches('/')))
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Online sağlayıcıya ulaşılamadı: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Sağlayıcı hata döndürdü ({}): {}", status, text));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Beklenmeyen yanıt formatı.".to_string())
}

#[tauri::command]
pub async fn online_chat(prompt: String, history: Vec<ChatMsgArg>) -> Result<String, String> {
    call_online(prompt, history).await
}

#[derive(Serialize, Deserialize)]
pub struct ChatMsgArg {
    pub role: String,
    pub content: String,
}
