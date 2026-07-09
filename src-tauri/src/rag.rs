// Amorfly AI — Aranabilir Belge Hafızası (RAG)
//
// "Geçen ay yüklediğim rapor neydi" gibi sorulara cevap verebilmek için
// analiz edilen belgeler, parçalara (chunk) bölünüp yerel Ollama embedding
// modeliyle (ör. nomic-embed-text — küçük, hızlı, ücretsiz) vektöre
// çevrilir ve yerel SQLite'a (rusqlite, bundled — sunucu/servis gerekmez)
// kaydedilir. Arama, kosinüs benzerliği ile Rust içinde hesaplanır —
// ayrı bir "vektör veritabanı" servisi kurmaya gerek yok, kişisel
// kullanım ölçeğinde (birkaç bin parça) brute-force yeterince hızlı.

use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

pub struct RagDb(pub Mutex<Connection>);

fn db_path() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Amorfly AI");
    dir.push("rag.sqlite");
    dir
}

pub fn init_db() -> Connection {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path).expect("RAG veritabanı açılamadı");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            chunk_text TEXT NOT NULL,
            embedding TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )
    .expect("tablo oluşturulamadı");
    conn
}

async fn embed(text: &str, model: &str) -> Result<Vec<f32>, String> {
    let client = crate::http_client_with_timeout(60);
    let body = serde_json::json!({ "model": model, "prompt": text });
    let res = client
        .post("http://127.0.0.1:11434/api/embeddings")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama embedding isteği başarısız: {}", e))?;

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let arr = json["embedding"]
        .as_array()
        .ok_or("Embedding alınamadı — model kurulu mu? (Modeller sekmesinden 'nomic-embed-text' indir)")?;
    Ok(arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
}

fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + size).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }
    chunks
}

fn default_model(m: &str) -> String {
    if m.is_empty() { "nomic-embed-text".to_string() } else { m.to_string() }
}

/// Bir belgeyi okuyup parçalara böler, her parçayı embedding'e çevirip
/// yerel veritabanına kaydeder — böylece daha sonra anlamsal olarak
/// aranabilir hale gelir.
#[tauri::command]
pub async fn index_document(db: State<'_, RagDb>, path: String, embed_model: String) -> Result<usize, String> {
    let text = crate::documents::extract_document_text(path.clone()).await?;
    let chunks = chunk_text(&text, 800, 100);
    let model = default_model(&embed_model);

    let mut count = 0;
    for chunk in &chunks {
        if chunk.trim().is_empty() {
            continue;
        }
        let vec = embed(chunk, &model).await?;
        let vec_json = serde_json::to_string(&vec).map_err(|e| e.to_string())?;
        let conn = db.0.lock().map_err(|_| "kilit hatası".to_string())?;
        conn.execute(
            "INSERT INTO chunks (path, chunk_text, embedding, created_at) VALUES (?1, ?2, ?3, datetime('now'))",
            params![path, chunk, vec_json],
        )
        .map_err(|e| format!("Kayıt eklenemedi: {}", e))?;
        count += 1;
    }

    crate::memory::remember(
        "islem".to_string(),
        format!("'{}' belgesini aranabilir hafızaya indeksledi ({} parça)", path, count),
    );
    Ok(count)
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return -1.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return -1.0;
    }
    dot / (na * nb)
}

#[derive(Serialize)]
pub struct SearchHit {
    pub path: String,
    pub snippet: String,
    pub score: f32,
}

/// Anlamsal (semantic) arama: sorguyu embedding'e çevirir, tüm kayıtlı
/// parçalarla kosinüs benzerliğini hesaplayıp en yakın olanları döner.
#[tauri::command]
pub async fn search_documents(db: State<'_, RagDb>, query: String, embed_model: String, top_k: usize) -> Result<Vec<SearchHit>, String> {
    let model = default_model(&embed_model);
    let qvec = embed(&query, &model).await?;

    let rows: Vec<(String, String, String)> = {
        let conn = db.0.lock().map_err(|_| "kilit hatası".to_string())?;
        let mut stmt = conn
            .prepare("SELECT path, chunk_text, embedding FROM chunks")
            .map_err(|e| e.to_string())?;
        let iter = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
            .map_err(|e| e.to_string())?;
        iter.filter_map(|r| r.ok()).collect()
    };

    let mut scored: Vec<SearchHit> = rows
        .into_iter()
        .filter_map(|(path, text, emb_json)| {
            let emb: Vec<f32> = serde_json::from_str(&emb_json).ok()?;
            let score = cosine(&qvec, &emb);
            Some(SearchHit { path, snippet: text.chars().take(240).collect(), score })
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    Ok(scored)
}

#[tauri::command]
pub fn clear_document_index(db: State<'_, RagDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "kilit hatası".to_string())?;
    conn.execute("DELETE FROM chunks", []).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn indexed_document_count(db: State<'_, RagDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|_| "kilit hatası".to_string())?;
    conn.query_row("SELECT COUNT(DISTINCT path) FROM chunks", [], |r| r.get(0))
        .map_err(|e| e.to_string())
}
