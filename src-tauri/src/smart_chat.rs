// Amorfly AI — Lokal / Hibrit / Online Mod Yönlendirici
//
// Üç çalışma modu, kullanıcının BİLGİÇLİ tercihiyle seçilir (arayüzdeki
// sağ üst çekmece), hiçbiri gizlice/otomatik seçilmez:
//
//   "lokal"  -> SADECE Amorfly'ın içindeki Ollama motoru. Hiçbir şey
//               dışarı çıkmaz. En gizli.
//   "hibrit" -> Önce yerel motor bir taslak üretir, sonra bu taslak
//               (ve orijinal istek) kayıtlı online sağlayıcıya
//               gönderilip gözden geçirtilir/geliştirilir. Bu modda
//               veri dışarı ÇIKAR (taslak + orijinal istek) — kullanıcı
//               bunu bilerek seçiyor, gizliliği "online" kadar
//               koruma iddiası taşımaz.
//   "online" -> Doğrudan kayıtlı online sağlayıcı (Groq vb.) cevap
//               verir, yerel motor hiç devreye girmez.
//
// Bu modül hem çok-turlu sohbet (ChatMessage listesi) hem tek-seferlik
// prompt (belge analizi, Excel üretici gibi) kullanım şekillerini
// destekler.

use crate::online::{call_online, ChatMsgArg};
use crate::{call_ollama, ChatMessage};

/// Çok turlu sohbet (Sohbet sekmesi, Dil Eğitimi) için.
#[tauri::command]
pub async fn smart_chat(mode: String, model: String, messages: Vec<ChatMessage>) -> Result<String, String> {
    if messages.is_empty() {
        return Err("Boş mesaj listesi.".to_string());
    }

    match mode.as_str() {
        "online" => {
            let (prompt, history) = split_last(&messages);
            call_online(prompt, history).await
        }
        "hibrit" => {
            let draft = call_ollama(model, messages.clone()).await?;
            let (orig_prompt, mut history) = split_last(&messages);
            history.push(ChatMsgArg { role: "user".to_string(), content: orig_prompt });
            let review_prompt = format!(
                "Aşağıda yerel bir yapay zeka modelinin ürettiği taslak cevap var. Kullanıcının \
                 son isteğini (sohbet geçmişinde) dikkate alarak bu taslağı gözden geçir, gerekirse \
                 düzelt/geliştir/tamamla. Sadece son, iyileştirilmiş cevabı ver — taslaktan ya da bu \
                 talimattan bahsetme.\n\n--- TASLAK ---\n{}\n--- TASLAK SONU ---",
                draft
            );
            call_online(review_prompt, history).await
        }
        _ => call_ollama(model, messages).await, // "lokal" ve tanınmayan değerler için güvenli varsayılan
    }
}

fn split_last(messages: &[ChatMessage]) -> (String, Vec<ChatMsgArg>) {
    let prompt = messages.last().map(|m| m.content.clone()).unwrap_or_default();
    let history = messages[..messages.len().saturating_sub(1)]
        .iter()
        .map(|m| ChatMsgArg { role: m.role.clone(), content: m.content.clone() })
        .collect();
    (prompt, history)
}

/// Tek seferlik prompt (belge analizi, Excel üretici) için — sohbet
/// geçmişi olmadan, doğrudan bir metin isteyip bir metin cevap alan
/// kullanım şekli. documents.rs ve excel_gen.rs bunu çağırır.
pub async fn run_single(mode: &str, model: &str, prompt: &str) -> Result<String, String> {
    match mode {
        "online" => call_online(prompt.to_string(), Vec::new()).await,
        "hibrit" => {
            let draft = call_ollama(
                model.to_string(),
                vec![ChatMessage { role: "user".to_string(), content: prompt.to_string() }],
            )
            .await?;
            let review_prompt = format!(
                "Aşağıda yerel bir modelin ürettiği taslak cevap var. Orijinal istek: \"{}\"\n\n\
                 Bu taslağı gözden geçir, gerekirse düzelt/geliştir. Sadece son hâli ver, taslaktan \
                 bahsetme.\n\n--- TASLAK ---\n{}\n--- TASLAK SONU ---",
                prompt, draft
            );
            call_online(review_prompt, Vec::new()).await
        }
        _ => {
            call_ollama(
                model.to_string(),
                vec![ChatMessage { role: "user".to_string(), content: prompt.to_string() }],
            )
            .await
        }
    }
}
