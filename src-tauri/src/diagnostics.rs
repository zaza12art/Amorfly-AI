// Amorfly AI — Tanılama (Diagnostics)
//
// Tüm bağımlılıkları tek ekranda yeşil/kırmızı gösterir. Ollama ve
// Video2X için gerçek (sudo istemeyen) otomatik kurulum zaten var
// (installer.rs) — bu ikisi "Otomatik Kur" alır. Diğerleri (ffmpeg,
// pandoc, poppler, libreoffice, xdotool) apt paketleri ve GUI'den
// sudo şifresi isteme imkansız olduğu için, bunun yerine bir terminal
// penceresi açıp `sudo apt install` komutunu oraya yazıyoruz —
// kullanıcı şifresini kendi açtığı terminale giriyor, biz asla görmüyoruz.

use crate::installer::amorfly_bin_dir;
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
pub struct CheckResult {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub auto_installable: bool, // installer.rs üzerinden sudo'suz kurulabilir
    pub apt_package: Option<String>, // apt ile kurulabiliyorsa paket adı
}

#[derive(Serialize)]
pub struct DiagnosticsReport {
    pub checks: Vec<CheckResult>,
    pub app_version: String,
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn run_diagnostics() -> DiagnosticsReport {
    let mut checks = Vec::new();

    let ollama_running = crate::http_client_with_timeout(5)
        .get("http://127.0.0.1:11434")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    checks.push(CheckResult {
        name: "Ollama".into(),
        ok: ollama_running,
        detail: if ollama_running { "Çalışıyor (127.0.0.1:11434)".into() } else { "Bulunamadı ya da çalışmıyor".into() },
        auto_installable: !ollama_running,
        apt_package: None,
    });

    let mut model_count = 0usize;
    if ollama_running {
        if let Ok(res) = crate::http_client_with_timeout(5).get("http://127.0.0.1:11434/api/tags").send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(arr) = json["models"].as_array() {
                    model_count = arr.len();
                }
            }
        }
    }
    checks.push(CheckResult {
        name: "İndirilmiş Model".into(),
        ok: model_count > 0,
        detail: format!("{} model bulundu", model_count),
        auto_installable: false,
        apt_package: None,
    });

    checks.push(CheckResult {
        name: "ffmpeg".into(),
        ok: which("ffmpeg"),
        detail: "Altyazı, dublaj, mikrofon kaydı ve seslendirme çalma için gerekli".into(),
        auto_installable: false,
        apt_package: Some("ffmpeg".into()),
    });

    checks.push(CheckResult {
        name: "zstd".into(),
        ok: which("zstd"),
        detail: "Ollama'nın otomatik (sudo'suz) kurulumu için gerekli (.tar.zst arşiv formatı)".into(),
        auto_installable: false,
        apt_package: Some("zstd".into()),
    });

    let libretranslate_running = crate::http_client_with_timeout(5)
        .get("http://127.0.0.1:5000/languages")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    checks.push(CheckResult {
        name: "LibreTranslate (opsiyonel)".into(),
        ok: libretranslate_running,
        detail: "Altyazı çevirisinde alternatif motor — özel eğitilmiş, genelde Ollama'dan daha \
                 tutarlı çeviri verir. Kurulum: docker run -ti -p 5000:5000 libretranslate/libretranslate"
            .into(),
        auto_installable: false,
        apt_package: None,
    });

    let video2x_ok = which("video2x") || amorfly_bin_dir().join("video2x.AppImage").exists();
    checks.push(CheckResult {
        name: "Video2X".into(),
        ok: video2x_ok,
        detail: "Video kalite artırma için gerekli".into(),
        auto_installable: !video2x_ok,
        apt_package: None,
    });

    checks.push(CheckResult {
        name: "whisper.cpp (whisper-cli)".into(),
        ok: which("whisper-cli"),
        detail: "Sesle tanıma ve altyazı üretimi için gerekli (github.com/ggerganov/whisper.cpp, derleyip PATH'e eklenmeli)".into(),
        auto_installable: false,
        apt_package: None,
    });

    let piper_ok = which("piper") || amorfly_bin_dir().join("piper").join("piper").exists();
    checks.push(CheckResult {
        name: "Piper TTS".into(),
        ok: piper_ok,
        detail: "Sesli okuma ve dublaj için gerekli, opsiyonel (github.com/rhasspy/piper)".into(),
        auto_installable: !piper_ok,
        apt_package: None,
    });

    checks.push(CheckResult {
        name: "pandoc".into(),
        ok: which("pandoc"),
        detail: "Word okuma, Word/PDF üretimi için gerekli".into(),
        auto_installable: false,
        apt_package: Some("pandoc".into()),
    });

    checks.push(CheckResult {
        name: "poppler-utils (pdftotext)".into(),
        ok: which("pdftotext"),
        detail: "PDF okuma için gerekli".into(),
        auto_installable: false,
        apt_package: Some("poppler-utils".into()),
    });

    checks.push(CheckResult {
        name: "LibreOffice".into(),
        ok: which("libreoffice"),
        detail: "PDF üretimi için gerekli".into(),
        auto_installable: false,
        apt_package: Some("libreoffice".into()),
    });

    checks.push(CheckResult {
        name: "xdotool".into(),
        ok: which("xdotool"),
        detail: "Kullanım alışkanlığı takibi için gerekli (yalnızca X11, Wayland'de çalışmaz)".into(),
        auto_installable: false,
        apt_package: Some("xdotool".into()),
    });

    checks.push(CheckResult {
        name: "tesseract-ocr".into(),
        ok: which("tesseract"),
        detail: "Toplu PDF OCR (Görevler sekmesi) için gerekli".into(),
        auto_installable: false,
        apt_package: Some("tesseract-ocr tesseract-ocr-tur".into()),
    });

    checks.push(CheckResult {
        name: "Python 3".into(),
        ok: which("python3"),
        detail: "Amorfly'ın çekirdek özellikleri için GEREKLİ DEĞİL — Rust/CLI araçları kullanıyoruz. Bazı ileri düzey harici modeller opsiyonel olarak isteyebilir.".into(),
        auto_installable: false,
        apt_package: Some("python3".into()),
    });

    DiagnosticsReport { checks, app_version: env!("CARGO_PKG_VERSION").to_string() }
}

/// Apt paketlerini kurmak için bir terminal penceresi açar (sudo şifresi
/// kullanıcının kendi açtığı terminalde istenir, uygulama asla görmez/tutmaz).
#[tauri::command]
pub fn open_terminal_install(apt_packages: Vec<String>, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        crate::logger::log_line("WARN", &format!("Onaysız terminal kurulum isteği reddedildi: {:?}", apt_packages));
        return Err("Onaylanmamış kurulum isteği — güvenlik nedeniyle çalıştırılmadı.".to_string());
    }
    crate::logger::log_line("INFO", &format!("Terminal kurulumu onaylandı: {:?}", apt_packages));

    // Öncelik: pkexec — polkit'in grafiksel şifre penceresini açar, terminal
    // gerektirmez, çok daha "tek tık" hissi verir. pkexec yoksa terminale düşer.
    if which("pkexec") {
        let mut cmd = Command::new("pkexec");
        cmd.arg("apt-get").arg("install").arg("-y").args(&apt_packages);
        match cmd.status() {
            Ok(status) if status.success() => {
                crate::logger::log_line("INFO", &format!("pkexec ile kuruldu: {:?}", apt_packages));
                return Ok(());
            }
            Ok(_) => {
                crate::logger::log_line("WARN", "pkexec ile kurulum iptal edildi ya da başarısız oldu, terminale düşülüyor.");
            }
            Err(e) => {
                crate::logger::log_line("WARN", &format!("pkexec çalıştırılamadı: {}, terminale düşülüyor.", e));
            }
        }
    }

    let pkg_list = apt_packages.join(" ");
    let cmd = format!(
        "sudo apt update && sudo apt install -y {}; echo; echo 'Kurulum bitti, bu pencereyi kapatabilirsin.'; exec bash",
        pkg_list
    );

    if which("gnome-terminal") {
        Command::new("gnome-terminal").args(["--", "bash", "-c", &cmd]).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    if which("konsole") {
        Command::new("konsole").args(["-e", "bash", "-c", &cmd]).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    if which("xfce4-terminal") {
        Command::new("xfce4-terminal").args(["-e", &format!("bash -c \"{}\"", cmd)]).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    if which("xterm") {
        Command::new("xterm").args(["-e", "bash", "-c", &cmd]).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }

    Err(format!(
        "Desteklenen bir terminal bulunamadı. Elle çalıştır: sudo apt install -y {}",
        pkg_list
    ))
}
