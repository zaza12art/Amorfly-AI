// Amorfly AI — Agent / Otomatik İş Akışı Planlayıcısı
//
// Kullanıcı "Bu videoyu Türkçeye çevir, dublaj yap ve 4K yap" gibi tek bir
// doğal dil isteği yazar. Yerel LLM, bunu SABİT ve BİLİNEN bir adım
// kümesinden (subtitle/dub/upscale) bir plana çevirir.
//
// ÖNEMLİ TASARIM KARARI: Model asla serbest metin/parametre üretmez,
// sadece önceden tanımlı adım kimliklerinden hangilerinin gerektiğine
// karar verir. Küçük yerel modeller (3B-7B) serbest planlamada
// güvenilmezken, sabit bir kelime dağarcığından seçim yapmakta oldukça
// başarılıdır. Dosya yollarının adımlar arası aktarımı (ör. altyazının
// dublaja girdi olması) TAMAMEN kod tarafından, deterministik şekilde
// yapılır — model bunu hiç görmez, hatalı yol uyduramaz.
//
// Plan kullanıcıya gösterilir, o onaylamadan (confirmed=true) hiçbir şey
// ÇALIŞTIRILMAZ. Onaylanan iş akışı, mevcut görev motoruna (tasks.rs)
// normal bir görev olarak eklenir — "Görevler" sekmesinde diğerleriyle
// birlikte görünür, iptal edilebilir, ilerlemesi aynı şekilde akar.

use crate::tasks::TaskStore;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

#[derive(Serialize, Deserialize, Clone)]
pub struct PlanStep {
    pub id: String,
    pub label: String,
}

const KNOWN_STEPS: &[(&str, &str)] = &[
    ("subtitle", "Türkçe altyazı üret (whisper.cpp + yerel model çevirisi)"),
    ("dub", "Türkçe dublaj üret (Piper TTS)"),
    ("upscale", "Video kalitesini artır (Video2X)"),
];

/// Doğal dil isteğini, bilinen adımlardan oluşan bir plana çevirir.
/// Model SADECE bu sabit listeden seçim yapar, serbest metin üretmez.
#[tauri::command]
pub async fn plan_workflow(goal: String, model: String) -> Result<Vec<PlanStep>, String> {
    let vocab: String = KNOWN_STEPS
        .iter()
        .map(|(id, desc)| format!("\"{}\" ({})", id, desc))
        .collect::<Vec<_>>()
        .join(", ");

    let prompt = format!(
        "Kullanıcının isteği: \"{}\"\n\n\
         Yalnızca şu adımlardan uygun olanları, doğru sırayla seç: {}.\n\
         SADECE bir JSON dizisi döndür, örnek: [\"subtitle\", \"dub\", \"upscale\"]. \
         Başka hiçbir açıklama, önsöz ya da metin ekleme. İstekte geçmeyen bir adımı asla ekleme. \
         'dub' (dublaj) her zaman 'subtitle'dan (altyazı) sonra gelmeli çünkü dublaj altyazıya ihtiyaç duyar.",
        goal, vocab
    );

    let client = crate::http_client_with_timeout(60);
    let body = serde_json::json!({
        "model": if model.is_empty() { "llama3.2".to_string() } else { model },
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
    let content = json["message"]["content"].as_str().unwrap_or("[]");

    let start = content.find('[').ok_or("Model geçerli bir plan üretemedi, tekrar dene ya da isteğini daha açık yaz.")?;
    let end = content.rfind(']').ok_or("Model geçerli bir plan üretemedi, tekrar dene ya da isteğini daha açık yaz.")?;
    let ids: Vec<String> = serde_json::from_str(&content[start..=end])
        .map_err(|_| "Plan çözümlenemedi, tekrar dene.".to_string())?;

    let steps: Vec<PlanStep> = ids
        .into_iter()
        .filter_map(|id| KNOWN_STEPS.iter().find(|(k, _)| *k == id).map(|(k, desc)| PlanStep { id: k.to_string(), label: desc.to_string() }))
        .collect();

    if steps.is_empty() {
        return Err(
            "İstekten bilinen bir iş akışı adımı çıkarılamadı. Şu an desteklenen adımlar: \
             altyazı, dublaj, kalite artırma. Daha açık yazmayı dene (ör. 'altyazı üret ve 4K yap')."
                .to_string(),
        );
    }
    Ok(steps)
}

/// Onaylanan planı mevcut görev motoruna yeni bir görev olarak ekler ve
/// adımları sırayla, dosya yolunu deterministik biçimde aktararak çalıştırır.
#[tauri::command]
pub async fn run_workflow(
    app: AppHandle,
    store: State<'_, TaskStore>,
    input_path: String,
    step_ids: Vec<String>,
    upscale_model: String,
    translation_model: String,
    piper_voice_model: String,
    whisper_bin: String,
    whisper_model_path: String,
    confirmed: bool,
) -> Result<String, String> {
    if !confirmed {
        return Err("Onaylanmamış iş akışı — güvenlik nedeniyle çalıştırılmadı.".to_string());
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let title = format!("İş akışı: {}", step_ids.join(" → "));
    crate::tasks::insert_manual_task(&store, &task_id, "workflow", &title);
    crate::tasks::emit_public(&app, &store);

    let store2 = store.inner().clone();
    let app2 = app.clone();
    let id2 = task_id.clone();

    tokio::spawn(async move {
        crate::tasks::set_status(&store2, &id2, "çalışıyor");
        crate::tasks::emit_public(&app2, &store2);

        let mut current_path = input_path.clone();
        let mut srt_path: Option<String> = None;
        let total = step_ids.len().max(1);

        for (i, step) in step_ids.iter().enumerate() {
            if crate::tasks::cancelled(&store2, &id2) {
                break;
            }
            crate::tasks::push_log(&store2, &id2, &format!("▶ {} başladı", step));
            crate::tasks::emit_public(&app2, &store2);

            let result: Result<(), String> = match step.as_str() {
                "subtitle" => {
                    match crate::subtitles::generate_turkish_subtitles(
                        app.clone(),
                        current_path.clone(),
                        whisper_bin.clone(),
                        whisper_model_path.clone(),
                        translation_model.clone(),
                        "ollama".to_string(),
                        "http://127.0.0.1:5000".to_string(),
                        String::new(),
                    )
                    .await
                    {
                        Ok(r) => {
                            srt_path = Some(r.srt_path);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                "dub" => match &srt_path {
                    None => Err("Önce altyazı üretilmeli (plan sırası hatalı).".to_string()),
                    Some(srt) => {
                        match crate::subtitles::generate_turkish_dub(
                            current_path.clone(),
                            srt.clone(),
                            "piper".to_string(),
                            piper_voice_model.clone(),
                        )
                        .await
                        {
                            Ok(out) => {
                                current_path = out;
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    }
                },
                "upscale" => match crate::upscale::upscale_video(app.clone(), current_path.clone(), 2, upscale_model.clone(), None).await {
                    Ok(r) => {
                        current_path = r.output_path;
                        Ok(())
                    }
                    Err(e) => Err(e),
                },
                other => Err(format!("Bilinmeyen adım: {}", other)),
            };

            if let Err(e) = result {
                crate::tasks::push_log(&store2, &id2, &format!("✗ {} hatası: {}", step, e));
                crate::tasks::fail_task(&store2, &id2, &e);
                crate::tasks::emit_public(&app2, &store2);
                crate::logger::log_line("ERROR", &format!("İş akışı adımı başarısız ({}): {}", step, e));
                return;
            }

            crate::tasks::push_log(&store2, &id2, &format!("✓ {} tamamlandı", step));
            crate::tasks::set_progress(&store2, &id2, ((i + 1) as f32 / total as f32) * 100.0);
            crate::tasks::emit_public(&app2, &store2);
        }

        let summary = format!("İş akışı tamamlandı. Son dosya: {}", current_path);
        crate::tasks::complete_task(&store2, &id2, &summary);
        crate::memory::remember("islem".to_string(), format!("Otomatik iş akışı çalıştırdı ({}): {}", step_ids.join("→"), current_path));
        crate::logger::log_line("INFO", &summary);
        crate::tasks::emit_public(&app2, &store2);
    });

    Ok(task_id)
}
