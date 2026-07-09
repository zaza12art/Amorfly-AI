// Amorfly AI — Belge Okuma/Analiz, Dışa Aktarma ve Görsel Analiz Modülü
//
// OKUMA (extract_document_text):
//   .xlsx/.xls -> calamine (saf Rust, harici araç gerekmez)
//   .pdf       -> pdftotext (poppler-utils)
//   .docx/.doc -> pandoc (metne çevirir)
//   .txt/.md   -> doğrudan okunur
//
// YAZMA (export_document):
//   txt  -> doğrudan yazılır
//   docx -> pandoc (markdown -> docx)
//   pdf  -> pandoc (markdown -> docx) + libreoffice --headless (docx -> pdf)
//           (LaTeX/weasyprint gibi ağır bağımlılıklar yerine, çoğu Linux
//           dağıtımında zaten kurulu olan LibreOffice kullanılıyor)
//   xlsx -> içerikteki markdown tablosu (varsa) satır/sütun olarak yazılır,
//           yoksa her satır tek sütuna yazılır (rust_xlsxwriter)
//
// GÖRSEL ANALİZ: Ollama'nın çok-modlu (vision) modelleri (ör. llava,
// llama3.2-vision) /api/chat üzerinden base64 görsel kabul eder.

use base64::{engine::general_purpose::STANDARD, Engine};
use calamine::{open_workbook_auto, Reader};
use rust_xlsxwriter::Workbook;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

// Önceden burada belgeler 14.000 karakterde SESSİZCE kesiliyordu (kullanıcı
// fark etmeden). Bunun yerine artık belge olduğu gibi modele gidiyor —
// gerçek bağlam penceresi hesaplaması artık smart_chat.rs -> call_ollama
// içinde merkezi olarak yapılıyor.

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn read_xlsx(path: &str) -> Result<String, String> {
    let mut wb = open_workbook_auto(path).map_err(|e| format!("Excel dosyası açılamadı: {}", e))?;
    let mut out = String::new();
    for sheet_name in wb.sheet_names().to_owned() {
        if let Ok(range) = wb.worksheet_range(&sheet_name) {
            out.push_str(&format!("## Sayfa: {}\n", sheet_name));
            for row in range.rows() {
                let cells: Vec<String> = row.iter().map(|c| c.to_string()).collect();
                out.push_str(&cells.join("\t"));
                out.push('\n');
            }
            out.push('\n');
        }
    }
    Ok(out)
}

async fn read_pdf(path: &str) -> Result<String, String> {
    let output = Command::new("pdftotext")
        .args(["-layout", path, "-"])
        .output()
        .await
        .map_err(|e| format!("pdftotext bulunamadı (poppler-utils gerekir): {}", e))?;
    if !output.status.success() {
        return Err("PDF okunamadı — şifreli ya da bozuk olabilir.".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn read_docx(path: &str) -> Result<String, String> {
    let output = Command::new("pandoc")
        .args([path, "-t", "plain"])
        .output()
        .await
        .map_err(|e| format!("pandoc bulunamadı: {}", e))?;
    if !output.status.success() {
        return Err("Word dosyası okunamadı.".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
pub async fn extract_document_text(path: String) -> Result<String, String> {
    let ext = ext_of(&path);
    let raw = match ext.as_str() {
        "xlsx" | "xls" | "xlsm" => read_xlsx(&path)?,
        "pdf" => read_pdf(&path).await?,
        "docx" | "doc" | "odt" => read_docx(&path).await?,
        "txt" | "md" | "csv" => {
            tokio::fs::read_to_string(&path).await.map_err(|e| format!("Dosya okunamadı: {}", e))?
        }
        other => return Err(format!("Desteklenmeyen dosya türü: .{}", other)),
    };
    Ok(raw)
}

/// Belgeyi okuyup yerel Ollama modeline soruyla birlikte gönderir.
#[tauri::command]
pub async fn analyze_document(path: String, question: String, model: String, mode: String) -> Result<String, String> {
    let _guard = crate::queue::acquire().await;
    let content = extract_document_text(path.clone()).await?;
    let prompt = format!(
        "Aşağıda bir belgenin içeriği var. Kullanıcının isteğini bu belgeye dayanarak, \
         net ve düzenli bir şekilde cevapla.\n\n--- BELGE İÇERİĞİ ---\n{}\n--- BELGE SONU ---\n\nİstek: {}",
        content, question
    );

    let answer = crate::smart_chat::run_single(&mode, &model, &prompt).await?;

    crate::memory::remember(
        "islem".to_string(),
        format!("'{}' dosyasını analiz etti — istek: \"{}\"", path, question),
    );
    Ok(answer)
}

/// Görseli çok-modlu (vision) bir Ollama modeline gönderir.
#[tauri::command]
pub async fn analyze_image(path: String, question: String, model: String) -> Result<String, String> {
    let _guard = crate::queue::acquire().await;
    let bytes = tokio::fs::read(&path).await.map_err(|e| format!("Görsel okunamadı: {}", e))?;
    let b64 = STANDARD.encode(bytes);

    let client = crate::http_client_with_timeout(180);
    let body = serde_json::json!({
        "model": if model.is_empty() { "llava:7b".to_string() } else { model },
        "messages": [{
            "role": "user",
            "content": question,
            "images": [b64],
        }],
        "options": { "num_ctx": 8192 },
        "stream": false,
    });
    let res = client.post("http://127.0.0.1:11434/api/chat").json(&body).send().await
        .map_err(|e| format!("Ollama'ya ulaşılamadı: {}", e))?;

    if !res.status().is_success() {
        return Err(
            "Görsel analizi başarısız. Seçili model çok-modlu (vision) olmayabilir — \
             Modeller sekmesinden 'llava' gibi görsel destekli bir model indirin.".to_string()
        );
    }

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    json["message"]["content"].as_str().map(|s| s.to_string())
        .ok_or_else(|| "Görsel analizi başarısız oldu.".to_string())
}

fn content_to_xlsx(content: &str, output_path: &str) -> Result<(), String> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    // Markdown tablosu var mı diye bak (| a | b | şeklinde satırlar)
    let table_lines: Vec<&str> = content.lines().filter(|l| l.trim_start().starts_with('|')).collect();

    if table_lines.len() >= 2 {
        for (r, line) in table_lines.iter().enumerate() {
            // ayırıcı satırı (---|---) atla
            if line.contains("---") { continue; }
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(|c| c.trim()).collect();
            for (c, cell) in cells.iter().enumerate() {
                let _ = sheet.write_string(r as u32, c as u16, *cell);
            }
        }
    } else {
        for (r, line) in content.lines().enumerate() {
            let _ = sheet.write_string(r as u32, 0, line);
        }
    }

    workbook.save(output_path).map_err(|e| format!("Excel dosyası kaydedilemedi: {}", e))?;
    Ok(())
}

/// İçeriği istenen formatta (txt/docx/pdf/xlsx) dışa aktarır.
#[tauri::command]
pub async fn export_document(content: String, format: String, output_path: String) -> Result<String, String> {
    match format.as_str() {
        "txt" => {
            tokio::fs::write(&output_path, &content).await.map_err(|e| format!("Yazılamadı: {}", e))?;
        }
        "xlsx" => {
            content_to_xlsx(&content, &output_path)?;
        }
        "docx" => {
            export_via_pandoc(&content, &output_path, "docx").await?;
        }
        "pdf" => {
            // md -> docx (pandoc) -> pdf (libreoffice), LaTeX/weasyprint bağımlılığı olmadan
            let tmp_docx = format!("{}.tmp.docx", output_path);
            export_via_pandoc(&content, &tmp_docx, "docx").await?;

            let out_dir = Path::new(&output_path).parent().and_then(|p| p.to_str()).unwrap_or(".");
            let status = Command::new("libreoffice")
                .args(["--headless", "--convert-to", "pdf", "--outdir", out_dir, &tmp_docx])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .map_err(|e| format!("libreoffice bulunamadı (PDF üretimi için gerekli): {}", e))?;

            let _ = tokio::fs::remove_file(&tmp_docx).await;

            if !status.success() {
                return Err("PDF oluşturulamadı.".to_string());
            }

            // libreoffice, giriş dosyasının (tmp_docx) uzantısını değiştirerek
            // çıktı üretir — yani "X.pdf.tmp.docx" -> "X.pdf.tmp.pdf" olur.
            // Bunu output_path'ten DEĞİL, tmp_docx'ten türetmek gerekiyor
            // (önceki sürümde bu karışmıştı, dosya bulunamıyordu).
            let produced = format!("{}.pdf", tmp_docx.trim_end_matches(".docx"));
            if Path::new(&produced).exists() {
                let _ = tokio::fs::rename(&produced, &output_path).await;
            }
        }
        other => return Err(format!("Desteklenmeyen dışa aktarma formatı: {}", other)),
    }
    Ok(output_path)
}

async fn export_via_pandoc(content: &str, output_path: &str, format: &str) -> Result<(), String> {
    let tmp_md = format!("{}.src.md", output_path);
    tokio::fs::write(&tmp_md, content).await.map_err(|e| format!("Geçici dosya yazılamadı: {}", e))?;

    let status = Command::new("pandoc")
        .args([&tmp_md, "-o", output_path, "-t", format])
        .status()
        .await
        .map_err(|e| format!("pandoc bulunamadı: {}", e))?;

    let _ = tokio::fs::remove_file(&tmp_md).await;

    if !status.success() {
        return Err(format!("{} dosyası oluşturulamadı.", format));
    }
    Ok(())
}
