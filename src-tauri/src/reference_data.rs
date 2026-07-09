// Amorfly AI — Mühendislik Referans Veritabanı
//
// Buradaki tüm sayılar wermac.org'un EN 10025-1/2 (EN 10365) standardına
// dayanan yayınlanmış tablolarından alınmıştır — AI'a "üretmedik", gerçek
// mühendislik kaynağından kopyaladık. Donatı ağırlık formülü de standart,
// yaygın kullanılan fiziksel formül (çelik yoğunluğu 7850 kg/m³ üzerinden).
//
// Neden statik/gömülü: mühendislik hesaplarında yanlış ağırlık/ölçü ciddi
// hatalara yol açabilir — bu yüzden bu veriler modele "tahmin ettirilmiyor",
// doğrudan burada sabit olarak duruyor.

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SteelProfile {
    pub series: String,
    pub name: String,
    pub height_mm: f64,
    pub width_mm: f64,
    pub web_mm: f64,
    pub flange_mm: f64,
    pub weight_kg_m: f64,
}

macro_rules! profile {
    ($series:expr, $size:expr, $h:expr, $w:expr, $tw:expr, $tf:expr, $wt:expr) => {
        SteelProfile {
            series: $series.to_string(),
            name: format!("{}{}", $series, $size),
            height_mm: $h,
            width_mm: $w,
            web_mm: $tw,
            flange_mm: $tf,
            weight_kg_m: $wt,
        }
    };
}

/// Kaynak: wermac.org — EN 10025-1/2 (IPE, INP, HEA, HEB, UNP, UPE).
pub fn all_steel_profiles() -> Vec<SteelProfile> {
    vec![
        // --- IPE --- (h, b, tw, tf, kg/m)
        profile!("IPE", 80, 80.0, 46.0, 3.8, 5.2, 6.11),
        profile!("IPE", 100, 100.0, 55.0, 4.1, 5.7, 8.26),
        profile!("IPE", 120, 120.0, 64.0, 4.4, 6.3, 10.6),
        profile!("IPE", 140, 140.0, 73.0, 4.7, 6.9, 13.1),
        profile!("IPE", 160, 160.0, 82.0, 5.0, 7.4, 16.1),
        profile!("IPE", 180, 180.0, 91.0, 5.3, 8.0, 19.2),
        profile!("IPE", 200, 200.0, 100.0, 5.6, 8.5, 22.8),
        profile!("IPE", 220, 220.0, 110.0, 5.9, 9.2, 26.7),
        profile!("IPE", 240, 240.0, 120.0, 6.2, 9.8, 31.3),
        profile!("IPE", 270, 270.0, 135.0, 6.6, 10.2, 36.8),
        profile!("IPE", 300, 300.0, 150.0, 7.1, 10.7, 43.0),
        profile!("IPE", 330, 330.0, 160.0, 7.5, 11.5, 50.1),
        profile!("IPE", 360, 360.0, 170.0, 8.0, 12.7, 58.2),
        profile!("IPE", 400, 400.0, 180.0, 8.6, 13.5, 67.6),
        profile!("IPE", 450, 450.0, 190.0, 9.4, 14.6, 79.1),
        profile!("IPE", 500, 500.0, 200.0, 10.2, 16.0, 92.4),
        profile!("IPE", 550, 550.0, 210.0, 11.1, 17.2, 108.0),
        profile!("IPE", 600, 600.0, 220.0, 12.0, 19.0, 125.0),
        // --- INP (IPN) ---
        profile!("INP", 80, 80.0, 42.0, 3.9, 5.9, 6.06),
        profile!("INP", 100, 100.0, 50.0, 4.5, 6.8, 8.50),
        profile!("INP", 120, 120.0, 58.0, 5.1, 7.7, 11.3),
        profile!("INP", 140, 140.0, 66.0, 5.7, 8.6, 14.6),
        profile!("INP", 160, 160.0, 74.0, 6.3, 9.5, 18.2),
        profile!("INP", 180, 180.0, 82.0, 6.9, 10.4, 22.3),
        profile!("INP", 200, 200.0, 90.0, 7.5, 11.3, 26.7),
        profile!("INP", 220, 220.0, 98.0, 8.1, 12.2, 31.6),
        profile!("INP", 240, 240.0, 106.0, 8.7, 13.1, 36.9),
        profile!("INP", 260, 260.0, 113.0, 9.4, 14.1, 42.7),
        profile!("INP", 280, 280.0, 119.0, 10.1, 15.2, 48.8),
        profile!("INP", 300, 300.0, 125.0, 10.8, 16.2, 55.2),
        profile!("INP", 320, 320.0, 131.0, 11.5, 17.3, 62.2),
        profile!("INP", 340, 340.0, 137.0, 12.2, 18.3, 69.3),
        profile!("INP", 360, 360.0, 143.0, 13.0, 19.5, 77.6),
        profile!("INP", 380, 380.0, 149.0, 13.7, 20.5, 85.6),
        profile!("INP", 400, 400.0, 155.0, 14.4, 21.6, 94.2),
        // --- HEA ---
        profile!("HEA", 100, 96.0, 100.0, 5.0, 8.0, 17.0),
        profile!("HEA", 120, 114.0, 120.0, 5.0, 8.0, 20.3),
        profile!("HEA", 140, 133.0, 140.0, 5.5, 8.5, 25.1),
        profile!("HEA", 160, 152.0, 160.0, 6.0, 9.0, 31.0),
        profile!("HEA", 180, 171.0, 180.0, 6.0, 9.5, 36.2),
        profile!("HEA", 200, 190.0, 200.0, 6.5, 10.0, 43.1),
        profile!("HEA", 220, 210.0, 220.0, 7.0, 11.0, 51.5),
        profile!("HEA", 240, 230.0, 240.0, 7.5, 12.0, 61.5),
        profile!("HEA", 260, 250.0, 260.0, 7.5, 12.5, 69.5),
        profile!("HEA", 280, 270.0, 280.0, 8.0, 13.0, 77.8),
        profile!("HEA", 300, 290.0, 300.0, 8.5, 14.0, 90.0),
        profile!("HEA", 320, 310.0, 300.0, 9.0, 15.5, 99.5),
        profile!("HEA", 340, 330.0, 300.0, 9.5, 16.5, 107.0),
        profile!("HEA", 360, 350.0, 300.0, 10.0, 17.5, 114.0),
        profile!("HEA", 400, 390.0, 300.0, 11.0, 19.0, 127.0),
        profile!("HEA", 450, 440.0, 300.0, 11.5, 21.0, 142.0),
        profile!("HEA", 500, 490.0, 300.0, 12.0, 23.0, 158.0),
        profile!("HEA", 550, 540.0, 300.0, 12.5, 24.0, 169.0),
        profile!("HEA", 600, 590.0, 300.0, 13.0, 25.0, 181.0),
        profile!("HEA", 650, 640.0, 300.0, 13.5, 26.0, 193.0),
        profile!("HEA", 700, 690.0, 300.0, 14.5, 27.0, 208.0),
        profile!("HEA", 800, 790.0, 300.0, 15.0, 28.0, 229.0),
        profile!("HEA", 900, 890.0, 300.0, 16.0, 30.0, 256.0),
        profile!("HEA", 1000, 990.0, 300.0, 16.5, 31.0, 277.0),
        // --- HEB ---
        profile!("HEB", 100, 100.0, 100.0, 6.0, 10.0, 20.8),
        profile!("HEB", 120, 120.0, 120.0, 6.5, 11.0, 27.2),
        profile!("HEB", 140, 140.0, 140.0, 7.0, 12.0, 34.4),
        profile!("HEB", 160, 160.0, 160.0, 8.0, 13.0, 43.4),
        profile!("HEB", 180, 180.0, 180.0, 8.5, 14.0, 52.2),
        profile!("HEB", 200, 200.0, 200.0, 9.0, 15.0, 62.5),
        profile!("HEB", 220, 220.0, 220.0, 9.5, 16.0, 72.8),
        profile!("HEB", 240, 240.0, 240.0, 10.0, 17.0, 84.8),
        profile!("HEB", 260, 260.0, 260.0, 10.0, 17.5, 94.8),
        profile!("HEB", 280, 280.0, 280.0, 10.5, 18.0, 105.0),
        profile!("HEB", 300, 300.0, 300.0, 11.0, 19.0, 119.0),
        profile!("HEB", 320, 320.0, 300.0, 11.5, 20.5, 129.0),
        profile!("HEB", 340, 340.0, 300.0, 12.0, 21.5, 137.0),
        profile!("HEB", 360, 360.0, 300.0, 12.5, 22.5, 145.0),
        profile!("HEB", 400, 400.0, 300.0, 13.5, 24.0, 158.0),
        profile!("HEB", 450, 450.0, 300.0, 14.0, 26.0, 174.0),
        profile!("HEB", 500, 500.0, 300.0, 14.5, 28.0, 191.0),
        profile!("HEB", 550, 550.0, 300.0, 15.0, 29.0, 203.0),
        profile!("HEB", 600, 600.0, 300.0, 15.5, 30.0, 216.0),
        profile!("HEB", 650, 650.0, 300.0, 16.0, 31.0, 229.0),
        profile!("HEB", 700, 700.0, 300.0, 17.0, 32.0, 245.0),
        profile!("HEB", 800, 800.0, 300.0, 17.5, 33.0, 267.0),
        profile!("HEB", 900, 900.0, 300.0, 18.5, 35.0, 297.0),
        profile!("HEB", 1000, 1000.0, 300.0, 19.0, 36.0, 320.0),
        // --- UNP ---
        profile!("UNP", 80, 80.0, 45.0, 6.0, 8.0, 8.82),
        profile!("UNP", 100, 100.0, 50.0, 6.0, 8.5, 10.8),
        profile!("UNP", 120, 120.0, 55.0, 7.0, 9.0, 13.6),
        profile!("UNP", 140, 140.0, 60.0, 7.0, 10.0, 16.3),
        profile!("UNP", 160, 160.0, 65.0, 7.5, 10.5, 19.2),
        profile!("UNP", 180, 180.0, 70.0, 8.0, 11.0, 22.4),
        profile!("UNP", 200, 200.0, 75.0, 8.5, 11.5, 25.7),
        profile!("UNP", 220, 220.0, 80.0, 9.0, 12.5, 30.0),
        profile!("UNP", 240, 240.0, 85.0, 9.5, 13.0, 33.8),
        profile!("UNP", 260, 260.0, 90.0, 10.0, 14.0, 38.6),
        profile!("UNP", 280, 280.0, 95.0, 10.0, 15.0, 42.7),
        profile!("UNP", 300, 300.0, 100.0, 10.0, 16.0, 47.0),
        profile!("UNP", 320, 320.0, 100.0, 14.0, 17.5, 60.6),
        profile!("UNP", 350, 350.0, 100.0, 14.0, 16.0, 61.8),
        profile!("UNP", 380, 380.0, 102.0, 13.5, 16.0, 64.3),
        profile!("UNP", 400, 400.0, 110.0, 14.0, 18.0, 73.2),
        // --- UPE ---
        profile!("UPE", 80, 80.0, 50.0, 4.5, 8.0, 9.05),
        profile!("UPE", 100, 100.0, 55.0, 5.0, 8.5, 11.1),
        profile!("UPE", 120, 120.0, 60.0, 5.5, 9.0, 13.5),
        profile!("UPE", 140, 140.0, 65.0, 6.0, 9.5, 16.0),
        profile!("UPE", 160, 160.0, 70.0, 6.5, 10.0, 19.0),
        profile!("UPE", 180, 180.0, 75.0, 7.0, 10.5, 22.0),
        profile!("UPE", 200, 200.0, 80.0, 7.5, 11.0, 25.3),
        profile!("UPE", 220, 220.0, 85.0, 8.0, 12.0, 29.4),
        profile!("UPE", 240, 240.0, 90.0, 8.5, 13.0, 34.0),
        profile!("UPE", 270, 270.0, 95.0, 9.0, 14.0, 39.5),
        profile!("UPE", 300, 300.0, 100.0, 9.5, 15.0, 45.3),
        profile!("UPE", 330, 330.0, 105.0, 11.0, 16.0, 54.2),
        profile!("UPE", 360, 360.0, 110.0, 12.0, 17.0, 62.3),
        profile!("UPE", 400, 400.0, 115.0, 13.5, 18.0, 73.5),
    ]
}

#[derive(Serialize, Clone)]
pub struct RebarSize {
    pub diameter_mm: f64,
    pub area_mm2: f64,
    pub weight_kg_m: f64,
}

/// Standart inşaat mühendisliği donatı ağırlık formülü:
/// ağırlık (kg/m) = çap² (mm) × 0.00617  — çelik yoğunluğu 7850 kg/m³
/// üzerinden türetilen, dünya genelinde kullanılan sabit katsayı.
pub fn rebar_table() -> Vec<RebarSize> {
    let diameters = [6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 25.0, 28.0, 32.0, 36.0, 40.0];
    diameters
        .iter()
        .map(|&d| RebarSize {
            diameter_mm: d,
            area_mm2: std::f64::consts::PI / 4.0 * d * d,
            weight_kg_m: 0.00617 * d * d,
        })
        .collect()
}
