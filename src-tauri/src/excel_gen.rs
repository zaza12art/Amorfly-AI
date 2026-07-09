// Amorfly AI — Akıllı Excel Üretici
//
// İki mod var:
// 1) export_reference_table: gömülü, doğrulanmış mühendislik verilerini
//    (çelik profilleri, donatı ağırlık tablosu) gerçek bir .xlsx'e döker.
//    Sayılar reference_data.rs'den geliyor — AI'a hiç sormuyoruz, çünkü
//    bunlar sabit, halüsinasyona kapalı olması gereken veriler.
// 2) generate_excel_from_description: kullanıcının serbest metinle
//    tarif ettiği bir hesap tablosunu ("donatı hesabı makrosu" gibi),
//    Ollama'ya yapı (JSON) olarak plan yaptırıp GERÇEK Excel formülleriyle
//    (statik sayı değil, hücre formülü) üretir.

use crate::reference_data::{all_steel_profiles, rebar_table};
use rust_xlsxwriter::{Color, Format, Workbook};
use serde::Deserialize;

#[tauri::command]
pub fn export_steel_profiles_excel(output_path: String) -> Result<String, String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Çelik Profilleri").map_err(|e| e.to_string())?;

    let header_fmt = Format::new().set_bold().set_background_color(Color::RGB(0xE95420)).set_font_color(Color::White);

    let headers = ["Seri", "Profil Adı", "Yükseklik (mm)", "Genişlik (mm)", "Gövde Kalınlığı (mm)", "Flanş Kalınlığı (mm)", "Ağırlık (kg/m)"];
    for (col, h) in headers.iter().enumerate() {
        worksheet.write_string_with_format(0, col as u16, *h, &header_fmt).map_err(|e| e.to_string())?;
        worksheet.set_column_width(col as u16, 20).map_err(|e| e.to_string())?;
    }

    let profiles = all_steel_profiles();
    for (i, p) in profiles.iter().enumerate() {
        let row = (i + 1) as u32;
        worksheet.write_string(row, 0, &p.series).map_err(|e| e.to_string())?;
        worksheet.write_string(row, 1, &p.name).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 2, p.height_mm).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 3, p.width_mm).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 4, p.web_mm).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 5, p.flange_mm).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 6, p.weight_kg_m).map_err(|e| e.to_string())?;
    }

    // Toplam metraj hesaplayabilmesi için "Adet" ve "Toplam Ağırlık (kg)"
    // sütunları — kullanıcı Adet'i doldurunca formül otomatik hesaplar.
    worksheet.write_string_with_format(0, 7, "Adet", &header_fmt).map_err(|e| e.to_string())?;
    worksheet.write_string_with_format(0, 8, "Toplam Ağırlık (kg)", &header_fmt).map_err(|e| e.to_string())?;
    worksheet.set_column_width(7, 12).map_err(|e| e.to_string())?;
    worksheet.set_column_width(8, 20).map_err(|e| e.to_string())?;
    for i in 0..profiles.len() {
        let row = (i + 1) as u32;
        worksheet.write_formula(row, 8, format!("=G{}*H{}", row + 1, row + 1).as_str()).map_err(|e| e.to_string())?;
    }

    workbook.save(&output_path).map_err(|e| format!("Excel kaydedilemedi: {}", e))?;
    crate::memory::remember("islem".to_string(), format!("Çelik profil referans tablosunu Excel'e aktardı: {}", output_path));
    Ok(output_path)
}

#[tauri::command]
pub fn export_rebar_table_excel(output_path: String) -> Result<String, String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Donatı Ağırlık Tablosu").map_err(|e| e.to_string())?;

    let header_fmt = Format::new().set_bold().set_background_color(Color::RGB(0xE95420)).set_font_color(Color::White);
    let headers = ["Çap (mm)", "Kesit Alanı (mm²)", "Birim Ağırlık (kg/m)", "Adet", "Boy (m)", "Toplam Ağırlık (kg)"];
    for (col, h) in headers.iter().enumerate() {
        worksheet.write_string_with_format(0, col as u16, *h, &header_fmt).map_err(|e| e.to_string())?;
        worksheet.set_column_width(col as u16, 18).map_err(|e| e.to_string())?;
    }

    let rebars = rebar_table();
    for (i, r) in rebars.iter().enumerate() {
        let row = (i + 1) as u32;
        let excel_row = row + 1; // 1-indexed Excel satır no (başlık=1)
        worksheet.write_number(row, 0, r.diameter_mm).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 1, r.area_mm2).map_err(|e| e.to_string())?;
        worksheet.write_number(row, 2, r.weight_kg_m).map_err(|e| e.to_string())?;
        // Adet ve Boy: kullanıcı dolduracak boş girdi hücreleri
        // Toplam Ağırlık: gerçek formül — Adet × Boy × Birim Ağırlık
        worksheet.write_formula(row, 5, format!("=D{}*E{}*C{}", excel_row, excel_row, excel_row).as_str()).map_err(|e| e.to_string())?;
    }

    workbook.save(&output_path).map_err(|e| format!("Excel kaydedilemedi: {}", e))?;
    crate::memory::remember("islem".to_string(), format!("Donatı ağırlık hesap tablosunu Excel'e aktardı: {}", output_path));
    Ok(output_path)
}

#[derive(Deserialize, Debug)]
struct ColumnSpec {
    header: String,
    /// "input"  -> kullanıcının dolduracağı boş hücre
    /// "formula" -> her satır için otomatik hesaplanan gerçek Excel formülü
    kind: String,
    /// {row} yerine gerçek Excel satır numarası konur, ör: "=B{row}*C{row}"
    formula: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SheetSpec {
    title: String,
    columns: Vec<ColumnSpec>,
    #[serde(default = "default_row_count")]
    row_count: usize,
}

fn default_row_count() -> usize {
    15
}

fn strip_markdown_fences(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")).unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim().to_string()
}

/// Kullanıcının doğal dille tarif ettiği bir tabloyu ("donatı hesabı
/// makrosu", "malzeme takip tablosu" vb.) Ollama'ya JSON plan yaptırıp
/// gerçek, formüllü bir .xlsx olarak üretir. Formülleri AI'ın kendisi
/// hesaplamıyor — sadece HANGİ formülün hangi sütuna gideceğine karar
/// veriyor, gerçek hesaplamayı (her satır için) Excel'in kendisi yapıyor.
#[tauri::command]
pub async fn generate_excel_from_description(
    description: String,
    model: String,
    output_path: String,
    mode: String,
) -> Result<String, String> {
    let _guard = crate::queue::acquire().await;

    let prompt = format!(
        "Bir mühendislik/ofis Excel tablosu tasarlayacaksın. Kullanıcının isteği: \"{}\"\n\n\
         SADECE aşağıdaki JSON şemasına uyan, başka hiçbir açıklama/metin içermeyen bir JSON döndür \
         (markdown kod bloğu ya da başka hiçbir şey ekleme, saf JSON):\n\
         {{\n\
         \"title\": \"Tablo başlığı\",\n\
         \"columns\": [\n\
         {{ \"header\": \"Sütun Adı\", \"kind\": \"input\" }},\n\
         {{ \"header\": \"Hesaplanan Sütun\", \"kind\": \"formula\", \"formula\": \"=B{{row}}*C{{row}}\" }}\n\
         ],\n\
         \"row_count\": 15\n\
         }}\n\n\
         Kurallar:\n\
         - \"kind\":\"input\" olan sütunlar kullanıcının elle dolduracağı boş hücrelerdir.\n\
         - \"kind\":\"formula\" olan sütunlarda \"formula\" alanı ZORUNLU, Excel formülü sözdiziminde \
         olmalı, hücre referansları harf+{{row}} şeklinde olmalı (örnek: \"=B{{row}}*C{{row}}*7.85\").\n\
         - Formülde SADECE gerçekten var olan (senin \"columns\" listesinde tanımladığın) sütunlara \
         atıfta bulun — olmayan bir sütuna (ör. sen 4 sütun tanımladıysan E ya da F'ye) ASLA atıf yapma, \
         bu bozuk/hatalı bir Excel dosyası üretir.\n\
         - Sütun harfleri soldan sağa A, B, C... şeklinde sırayla verilir, sen sadece doğru harfi kullan.\n\
         - GERÇEKÇİ OL, EN AZ SAYIDA SÜTUNLA GEÇİŞTİRME: bu tablo gerçek bir işte kullanılacak. Örneğin \
         bir MALİYET/FİYAT ANALİZİ tablosu istenirse en az şu sütunlar olmalı: İş Kalemi (input), Birim \
         (input), Miktar (input), Birim Fiyat (input), Toplam Tutar (formula: Miktar×Birim Fiyat). \"En az \
         2 sütun yeter\" diye düşünme — eksik/yetersiz bir tablo kullanıcıya hiçbir fayda sağlamaz.\n\
         - En az 4, en fazla 10 sütun olsun. row_count 10-30 arası makul bir sayı olsun.\n\
         - Fiziksel/mühendislik formülü gerekiyorsa (ağırlık, hacim, alan vb.) doğru, standart formülü kullan.",
        description
    );

    // Yerel modelin JSON'u düzyazıyla sarmalama riski daha yüksek olduğu
    // için "lokal" modda Ollama'nın strict "format: json" özelliğini
    // koruyoruz (online sağlayıcılar bunu aynı şekilde desteklemeyebilir,
    // o yüzden hibrit/online'da genel smart_chat yoluna düşüyoruz).
    let raw = if mode == "hibrit" || mode == "online" {
        crate::smart_chat::run_single(&mode, &model, &prompt).await?
    } else {
        let client = crate::http_client_with_timeout(300);
        let num_ctx = ((prompt.len() / 3) as u32 + 512).clamp(2048, 131072);
        let body = serde_json::json!({
            "model": if model.is_empty() { "llama3.2".to_string() } else { model },
            "messages": [{ "role": "user", "content": prompt }],
            "options": { "num_ctx": num_ctx },
            "stream": false,
            "format": "json",
        });
        let res = client
            .post("http://127.0.0.1:11434/api/chat")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama'ya ulaşılamadı: {}", e))?;
        let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        json["message"]["content"]
            .as_str()
            .ok_or_else(|| "Model boş yanıt döndürdü.".to_string())?
            .to_string()
    };

    let cleaned = strip_markdown_fences(&raw);
    let spec: SheetSpec = serde_json::from_str(&cleaned)
        .map_err(|e| format!("Model geçerli bir tablo planı üretemedi ({}). Farklı bir model deneyin ya da isteği daha net yazın.", e))?;

    if spec.columns.is_empty() {
        return Err("Model hiç sütun üretmedi.".to_string());
    }

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let _ = worksheet.set_name(&truncate_sheet_name(&spec.title));

    let header_fmt = Format::new().set_bold().set_background_color(Color::RGB(0xE95420)).set_font_color(Color::White);

    for (col, c) in spec.columns.iter().enumerate() {
        worksheet.write_string_with_format(0, col as u16, &c.header, &header_fmt).map_err(|e| e.to_string())?;
        worksheet.set_column_width(col as u16, 22).map_err(|e| e.to_string())?;
    }

    let row_count = spec.row_count.clamp(1, 500);
    for r in 0..row_count {
        let row = (r + 1) as u32;
        let excel_row = row + 1; // Excel'de 1. satır başlık, veri 2'den başlar
        for (col, c) in spec.columns.iter().enumerate() {
            if c.kind == "formula" {
                if let Some(f) = &c.formula {
                    // Formül, tabloda gerçekten var olmayan bir sütuna atıfta
                    // bulunuyorsa YAZMA — bu, kullanıcının "Hata 000" gibi bozuk
                    // bir Excel hatasıyla karşılaşmasının ana sebebiydi. Bunun
                    // yerine hücre boş kalır, en azından dosya bozulmaz.
                    let out_of_range = max_referenced_column_index(f)
                        .map(|idx| idx >= spec.columns.len())
                        .unwrap_or(false);
                    if !out_of_range {
                        let resolved = f.replace("{row}", &excel_row.to_string());
                        worksheet.write_formula(row, col as u16, resolved.as_str()).map_err(|e| e.to_string())?;
                    }
                }
            }
            // "input" sütunları bilerek boş bırakılıyor — kullanıcı dolduracak.
        }
    }

    workbook.save(&output_path).map_err(|e| format!("Excel kaydedilemedi: {}", e))?;
    crate::memory::remember("islem".to_string(), format!("AI ile Excel tablosu üretti: \"{}\" -> {}", description, output_path));

    Ok(output_path)
}

/// Modelin ürettiği formülde atıf yapılan EN BÜYÜK sütun indeksini bulur
/// (A=0, B=1, ... Z=25, AA=26...). "Hata 000" gibi bozuk Excel hatalarının
/// gerçek sebebi genelde şuydu: model 4 sütunluk bir tablo tanımlayıp
/// formülde var olmayan E/F sütununa atıfta bulunuyordu — Excel bu durumda
/// hata veriyor. Artık yazmadan önce bunu kontrol ediyoruz.
fn max_referenced_column_index(formula: &str) -> Option<usize> {
    let chars: Vec<char> = formula.chars().collect();
    let mut max_idx: Option<usize> = None;
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_uppercase() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_uppercase() {
                i += 1;
            }
            let letters: String = chars[start..i].iter().collect();
            // Harf grubunun hemen ardından "{row}" ya da bir rakam geliyorsa
            // bu gerçek bir hücre referansıdır (fonksiyon adı değil, ör. "SUM").
            let rest: String = chars[i..].iter().collect();
            if rest.starts_with("{row}") || rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                let mut col_idx: i64 = -1;
                for ch in letters.chars() {
                    col_idx = (col_idx + 1) * 26 + (ch as i64 - 'A' as i64);
                }
                if col_idx >= 0 {
                    let idx = col_idx as usize;
                    max_idx = Some(max_idx.map_or(idx, |m: usize| m.max(idx)));
                }
            }
        } else {
            i += 1;
        }
    }
    max_idx
}

fn truncate_sheet_name(title: &str) -> String {
    // Excel sayfa adları en fazla 31 karakter ve bazı özel karakterleri kabul etmiyor.
    let cleaned: String = title.chars().filter(|c| !"[]:*?/\\".contains(*c)).collect();
    if cleaned.chars().count() > 31 {
        cleaned.chars().take(31).collect()
    } else if cleaned.is_empty() {
        "Tablo".to_string()
    } else {
        cleaned
    }
}
