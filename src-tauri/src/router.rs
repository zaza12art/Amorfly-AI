// Amorfly AI — Basit AI Router
//
// Fikir: her görev aynı modelle aynı kalitede yapılmıyor (kod sorusu için
// iyi bir genel model yeterli olmayabilir, qwen2.5-coder gibi kod-odaklı
// bir model çok daha tutarlı sonuç verir). Bu modül "hangi görevde hangi
// model" eşleşmesini kullanıcının kendi kurduğu modellere göre ÖNERİR ve
// kaydeder — otomatik indirme/zorlama YAPMAZ, sadece hazır olanlar
// arasından en uygununu önerir, kullanıcı her zaman elle değiştirebilir.

use crate::security::{vault_read, vault_write};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct RouterConfig {
    pub genel: String,
    pub kod: String,
    pub belge_analiz: String,
    pub excel: String,
    pub dil_egitimi: String,
    pub gorsel_analiz: String,
}

impl Default for RouterConfig {
    fn default() -> Self {
        RouterConfig {
            genel: "llama3.2".to_string(),
            kod: "qwen2.5-coder:7b".to_string(),
            belge_analiz: "llama3.2".to_string(),
            excel: "llama3.2".to_string(),
            dil_egitimi: "llama3.2".to_string(),
            gorsel_analiz: "llava:7b".to_string(),
        }
    }
}

const ROUTER_KEY: &str = "ai_router_config";

#[tauri::command]
pub fn get_router_config() -> Result<RouterConfig, String> {
    match vault_read(ROUTER_KEY.to_string())? {
        Some(json) => serde_json::from_str(&json).map_err(|e| e.to_string()),
        None => Ok(RouterConfig::default()),
    }
}

#[tauri::command]
pub fn save_router_config(config: RouterConfig) -> Result<(), String> {
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    vault_write(ROUTER_KEY.to_string(), json)
}

/// Kurulu modeller arasında görev için en uygun olanı ÖNERİR. Kullanıcının
/// router ayarlarında elle seçtiği model varsa ve hâlâ kuruluysa o
/// kullanılır; yoksa kurulu modeller arasında görev için makul bir
/// varsayılana (örn. "coder" geçen bir model kod görevine) düşer, o da
/// yoksa ilk kurulu modele düşer.
#[tauri::command]
pub fn suggest_model_for_task(task: String, installed_models: Vec<String>) -> Result<String, String> {
    if installed_models.is_empty() {
        return Err("Hiç model kurulu değil. Önce Modeller sekmesinden bir model indir.".to_string());
    }

    let config = get_router_config()?;
    let preferred = match task.as_str() {
        "kod" => &config.kod,
        "belge_analiz" => &config.belge_analiz,
        "excel" => &config.excel,
        "dil_egitimi" => &config.dil_egitimi,
        "gorsel_analiz" => &config.gorsel_analiz,
        _ => &config.genel,
    };

    // ÖNEMLİ DÜZELTME: Ollama modelleri genelde ":latest" etiketiyle
    // listeleniyor (ör. "llama3.2:latest") ama varsayılan tercihlerimiz
    // etiketsiz ("llama3.2") yazılı. Birebir string eşleşmesi (==) bu
    // yüzden SESSİZCE başarısız oluyordu — ve "dil_egitimi" gibi anahtar
    // kelime yedeği olmayan görevlerde, hangi model olursa olsun kurulu
    // İLK modele düşülüyordu. Bu, şansa göre embedding-only bir model
    // (ör. nomic-embed-text) olabiliyordu — o da sohbet edemediği için
    // "does not support chat" hatasıyla çöküyordu. Artık etiketi göz ardı
    // eden bir karşılaştırma yapıyoruz (baştan eşleşme yeterli).
    let base_name = |m: &str| m.split(':').next().unwrap_or(m).to_string();
    let preferred_base = base_name(preferred);
    if let Some(found) = installed_models.iter().find(|m| base_name(m) == preferred_base) {
        return Ok(found.clone());
    }

    // Tercih edilen kurulu değil — görev tipine göre kurulu modeller
    // arasında isimden makul bir tahmin yap.
    let keyword = match task.as_str() {
        "kod" => Some("coder"),
        "gorsel_analiz" => Some("llava"),
        _ => None,
    };
    if let Some(kw) = keyword {
        if let Some(found) = installed_models.iter().find(|m| m.to_lowercase().contains(kw)) {
            return Ok(found.clone());
        }
    }

    // Son çare yedeği: SADECE gerçek sohbet edebilen (embedding-only
    // olmayan) modeller arasından seç — "embed" geçen modeller (ör.
    // nomic-embed-text) hiçbir zaman sohbet için önerilmemeli, kurulu
    // ilk sırada olsalar bile.
    if let Some(chat_capable) = installed_models.iter().find(|m| !m.to_lowercase().contains("embed")) {
        return Ok(chat_capable.clone());
    }

    Ok(installed_models[0].clone())
}
