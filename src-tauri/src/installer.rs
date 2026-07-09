// Amorfly AI — Taşınabilir (sudo gerektirmeyen) Otomatik Kurulum
//
// Ollama ve Video2X, sistem genelinde (systemd/root) kurulum yerine
// ~/.local/share/amorfly/bin altına indirilip oradan çalıştırılır. Bu
// hem "kayıtsız/bağımsız" felsefesine uyar hem de GUI uygulamasından
// sudo şifresi isteme sorununu (terminal olmadığı için imkansız) ortadan
// kaldırır.
//
// GROQ İÇİN BİLİNÇLİ OLARAK YAPILMAYAN ŞEY: Groq'a "arka planda otomatik
// oturum açma" eklenmedi. Bu, üçüncü taraf bir sitenin giriş formunu
// otomatikleştirmek anlamına gelir — güvensizdir (parolanı bir yerde
// tutmamız gerekir) ve çoğu servisin kullanım şartlarına aykırıdır.
// Bunun yerine sistem tarayıcısında ilgili sayfa açılır (`open_url`),
// kullanıcı kendi tercih ettiği yöntemle (Google/GitHub/e-posta) giriş
// yapar ve API anahtarını kendi eliyle yapıştırır.

use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub fn amorfly_bin_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Amorfly AI");
    dir.push("bin");
    dir
}

#[derive(Serialize)]
pub struct InstallResult {
    pub installed_path: String,
    pub already_running: bool,
}

#[cfg(unix)]
fn make_executable(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(path, perms);
    }
}

/// Ollama'yı sudo/systemd olmadan kullanıcı dizinine indirir, açar ve
/// arka planda `ollama serve` olarak başlatır.
#[tauri::command]
pub async fn install_ollama_portable() -> Result<InstallResult, String> {
    crate::logger::log_line("INFO", "Ollama taşınabilir kurulum başlatıldı");
    let bin_dir = amorfly_bin_dir();
    fs::create_dir_all(&bin_dir).map_err(|e| format!("Dizin oluşturulamadı: {}", e))?;

    let ollama_bin = bin_dir.join("bin").join("ollama");

    if !ollama_bin.exists() {
        // 2026 itibarıyla Ollama'nın resmi Linux paketi .tgz değil .tar.zst
        // formatında dağıtılıyor (zstd sıkıştırma). Eski .tgz linki artık
        // güncel değil/güvenilmez — bu yüzden indirme "sıfır" görünüyordu.
        let archive_path = bin_dir.join("ollama-linux-amd64.tar.zst");
        let bytes = reqwest::get("https://ollama.com/download/ollama-linux-amd64.tar.zst")
            .await
            .map_err(|e| format!("İndirme başarısız (internet bağlantını kontrol et): {}", e))?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        fs::write(&archive_path, &bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;

        // --zstd bayrağı modern GNU tar'da (Ubuntu 24.04+ zaten kurulu) var.
        // zstd komutu sistemde yoksa tar bunu bildirir, hata mesajında belirtiyoruz.
        let status = Command::new("tar")
            .args(["--zstd", "-xf", archive_path.to_str().unwrap_or(""), "-C", bin_dir.to_str().unwrap_or(".")])
            .status()
            .await
            .map_err(|e| format!("'tar' komutu çalıştırılamadı: {}", e))?;

        let _ = fs::remove_file(&archive_path);

        if !status.success() || !ollama_bin.exists() {
            return Err(
                "Ollama arşivi açılamadı. Muhtemel neden: sistemde 'zstd' paketi eksik \
                 (çözüm: terminalde 'sudo apt install zstd' çalıştır) ya da Ollama sıkıştırma \
                 formatını tekrar değiştirmiş olabilir. Elle kurulum: ollama.com/download".to_string()
            );
        }
    }

    make_executable(&ollama_bin);

    let already = crate::http_client_with_timeout(5)
        .get("http://127.0.0.1:11434")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if !already {
        Command::new(&ollama_bin)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                crate::logger::log_line("ERROR", &format!("Ollama başlatılamadı: {}", e));
                format!("Ollama başlatılamadı: {}", e)
            })?;
    }

    crate::logger::log_line("INFO", "Ollama kurulumu/başlatması tamamlandı");
    Ok(InstallResult {
        installed_path: ollama_bin.to_string_lossy().to_string(),
        already_running: already,
    })
}

#[derive(serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize)]
struct GhRelease {
    assets: Vec<GhAsset>,
}

/// Video2X'in en son Linux AppImage sürümünü GitHub'dan indirir
/// (herkese açık, hesap gerektirmeyen bir indirme — kayıt yok).
#[tauri::command]
pub async fn install_video2x_portable() -> Result<InstallResult, String> {
    let bin_dir = amorfly_bin_dir();
    fs::create_dir_all(&bin_dir).map_err(|e| format!("Dizin oluşturulamadı: {}", e))?;
    let appimage_path = bin_dir.join("video2x.AppImage");

    if !appimage_path.exists() {
        let client = reqwest::Client::builder()
            .user_agent("amorfly-ai")
            .build()
            .map_err(|e| e.to_string())?;

        let release: GhRelease = client
            .get("https://api.github.com/repos/k4yt3x/video2x/releases/latest")
            .send()
            .await
            .map_err(|e| format!("GitHub'a ulaşılamadı: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Sürüm bilgisi okunamadı: {}", e))?;

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.to_lowercase().contains("x86_64") && a.name.to_lowercase().ends_with(".appimage"))
            .ok_or("Uygun bir Linux AppImage dosyası bulunamadı (proje dosya isimlerini değiştirmiş olabilir, github.com/k4yt3x/video2x/releases sayfasından elle indirebilirsin).")?;

        let bytes = client
            .get(&asset.browser_download_url)
            .send()
            .await
            .map_err(|e| format!("İndirme başarısız: {}", e))?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;

        fs::write(&appimage_path, &bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;
    }

    make_executable(&appimage_path);

    Ok(InstallResult {
        installed_path: appimage_path.to_string_lossy().to_string(),
        already_running: false,
    })
}

/// Piper TTS'in en son Linux (x86_64) sürümünü GitHub'dan indirir ve
/// çalıştırılabilir hale getirir — hesap/kayıt gerekmez.
#[tauri::command]
pub async fn install_piper_portable() -> Result<InstallResult, String> {
    let bin_dir = amorfly_bin_dir();
    fs::create_dir_all(&bin_dir).map_err(|e| format!("Dizin oluşturulamadı: {}", e))?;
    let piper_bin = bin_dir.join("piper").join("piper");

    if !piper_bin.exists() {
        let client = reqwest::Client::builder()
            .user_agent("amorfly-ai")
            .build()
            .map_err(|e| e.to_string())?;

        let release: GhRelease = client
            .get("https://api.github.com/repos/rhasspy/piper/releases/latest")
            .send()
            .await
            .map_err(|e| format!("GitHub'a ulaşılamadı: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Sürüm bilgisi okunamadı: {}", e))?;

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.to_lowercase().contains("linux") && a.name.to_lowercase().contains("x86_64"))
            .ok_or("Uygun bir Linux (x86_64) paketi bulunamadı — github.com/rhasspy/piper/releases sayfasından elle indirebilirsin.")?;

        let tar_path = bin_dir.join("piper.tar.gz");
        let bytes = client
            .get(&asset.browser_download_url)
            .send()
            .await
            .map_err(|e| format!("İndirme başarısız: {}", e))?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        fs::write(&tar_path, &bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;

        let status = Command::new("tar")
            .args(["-C", bin_dir.to_str().unwrap_or("."), "-xzf", tar_path.to_str().unwrap_or("")])
            .status()
            .await
            .map_err(|e| format!("'tar' komutu çalıştırılamadı: {}", e))?;
        let _ = fs::remove_file(&tar_path);

        if !status.success() || !piper_bin.exists() {
            return Err("Piper arşivi açılamadı ya da beklenen dosya yapısı farklı — elle kurulum gerekebilir.".to_string());
        }
    }

    make_executable(&piper_bin);

    Ok(InstallResult {
        installed_path: piper_bin.to_string_lossy().to_string(),
        already_running: false,
    })
}

/// Piper için Türkçe ses modelini (onnx + config) Hugging Face'ten indirir
/// (herkese açık, kayıt gerekmeyen bir barındırma — model dosyaları özgürce
/// paylaşılıyor).
#[tauri::command]
pub async fn download_piper_turkish_voice() -> Result<String, String> {
    let bin_dir = amorfly_bin_dir();
    let voices_dir = bin_dir.join("piper-voices");
    fs::create_dir_all(&voices_dir).map_err(|e| format!("Dizin oluşturulamadı: {}", e))?;

    let base = "https://huggingface.co/rhasspy/piper-voices/resolve/main/tr/tr_TR/dfki/medium";
    let onnx_path = voices_dir.join("tr_TR-dfki-medium.onnx");
    let json_path = voices_dir.join("tr_TR-dfki-medium.onnx.json");

    for (url_suffix, out_path) in [
        ("tr_TR-dfki-medium.onnx", &onnx_path),
        ("tr_TR-dfki-medium.onnx.json", &json_path),
    ] {
        if out_path.exists() {
            continue;
        }
        let url = format!("{}/{}", base, url_suffix);
        let bytes = reqwest::get(&url)
            .await
            .map_err(|e| format!("Ses modeli indirilemedi: {}", e))?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        fs::write(out_path, &bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;
    }

    Ok(onnx_path.to_string_lossy().to_string())
}
