// Amorfly AI — Video Kalite Artırma (Upscale) Modülü
//
// video2x CLI'ye köprü kurar (github.com/k4yt3x/video2x). Bu araç birden
// fazla AI motorunu tek çatı altında birleştiriyor:
//   - RealESRGAN_x4plus      -> gerçek video (kamera, telefon, VHS) için
//   - realesr-animevideov3   -> anime/çizim için (Real-ESRGAN'ın anime sürümü)
//   - realesrgan-plus-anime  -> hibrit içerik
//   - RIFE                   -> çözünürlük değil, KARE HIZI (fps) artırır
//
// Video2X, Vulkan destekli bir GPU gerektirir (NVIDIA/AMD/Intel fark etmez).
//
// BİLİNÇLİ TASARIM KARARI: Dosya boyutuna hiçbir üst sınır KONULMAMIŞTIR.
// Ne kadar büyük/uzun bir video olursa olsun kullanıcı işleyebilir; süre
// donanıma bağlıdır, yazılım tarafında yapay bir kısıtlama yoktur.

use crate::installer::amorfly_bin_dir;
use serde::Serialize;
use std::process::Stdio;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;
use tokio::time::Duration;

/// subtitles.rs'deki aynı düzeltme — video2x gibi çok ağır, uzun süren
/// işlemler DÜŞÜK öncelikte (nice) çalıştırılmazsa masaüstü tamamen
/// tepkisiz hale gelebiliyor ("mouse bile oynamıyor" raporu). CPU'yu hâlâ
/// kullanır, işi hâlâ bitirir, ama masaüstüne öncelik tanınır.
fn nice_command(program: &str) -> Command {
    let mut cmd = Command::new("nice");
    cmd.arg("-n").arg("15").arg(program);
    cmd
}

/// subtitles.rs'deki aynı güvenlik kontrolü — bkz. orada detaylı açıklama.
fn available_memory_mb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb: u64 = rest.trim().split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

fn check_memory_or_warn() -> Result<(), String> {
    const MIN_SAFE_MEMORY_MB: u64 = 2048;
    if let Some(available) = available_memory_mb() {
        if available < MIN_SAFE_MEMORY_MB {
            return Err(format!(
                "Boş bellek kritik derecede az ({} MB). video2x bu durumda çalışırsa sistemin \
                 kilitlenmesine yol açabilir. Lütfen önce diğer ağır uygulamaları kapatıp tekrar dene.",
                available
            ));
        }
    }
    Ok(())
}

/// video2x binary'sini önce PATH'te arar, yoksa Amorfly'ın taşınabilir
/// kurulum dizinindeki (~/.local/share/amorfly/bin/video2x.AppImage)
/// sürümü kullanır.
async fn resolve_video2x_binary() -> Result<String, String> {
    let which = Command::new("which").arg("video2x").output().await;
    if let Ok(out) = which {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }

    let portable = amorfly_bin_dir().join("video2x.AppImage");
    if portable.exists() {
        return Ok(portable.to_string_lossy().to_string());
    }

    Err(
        "video2x bulunamadı. 'Video2X'i Otomatik Kur' butonunu kullanın ya da elle kurun: \
         github.com/k4yt3x/video2x".to_string()
    )
}

#[derive(Serialize)]
pub struct GpuOption {
    pub index: u32,
    pub name: String,
}

/// `video2x --list-gpus` çıktısını parse eder.
#[tauri::command]
pub async fn list_upscale_gpus() -> Result<Vec<GpuOption>, String> {
    let bin = resolve_video2x_binary().await?;
    let output = Command::new(&bin)
        .arg("--list-gpus")
        .output()
        .await
        .map_err(|e| format!("video2x çalıştırılamadı: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();
    for line in text.lines() {
        // Örnek satır: "0. NVIDIA RTX A6000"
        if let Some((idx_str, name)) = line.split_once('.') {
            if let Ok(idx) = idx_str.trim().parse::<u32>() {
                gpus.push(GpuOption { index: idx, name: name.trim().to_string() });
            }
        }
    }
    Ok(gpus)
}

#[derive(Serialize)]
pub struct UpscaleResult {
    pub output_path: String,
}

#[derive(Serialize, Clone)]
pub struct UpscaleProgress {
    pub status: String,
    pub elapsed_secs: u64,
    pub done: bool,
}

/// Videoyu AI ile büyütür. Dosya boyutuna/uzunluğuna hiçbir sınır
/// koymuyoruz — bilinçli bir tasarım kararı, süre tamamen donanıma bağlı.
///
/// ÖNEMLİ DÜZELTME: video2x'in çıktısı önceden tamamen çöpe atılıyordu
/// (Stdio::null()) ve işlem bitene kadar (video uzun/donanım zayıfsa
/// dakikalarca, hatta saatlerce) HİÇBİR geri bildirim verilmiyordu —
/// kullanıcıya "donmuş" gibi görünüyordu. Video2x'in kendi ilerleme
/// çıktısının formatı garanti/dokümante olmadığı için onu ayrıştırmaya
/// güvenmek yerine, işlemle PARALEL çalışan bir "kalp atışı" (heartbeat)
/// görevi ekledik: her birkaç saniyede bir "hâlâ çalışıyor, X saniye
/// geçti" olayı yayınlıyor. Bu, gerçek yüzde vermese de uygulamanın
/// gerçekten çalıştığını kanıtlıyor.
#[tauri::command]
pub async fn upscale_video(
    app: AppHandle,
    video_path: String,
    scale: u32,          // 2 ya da 4
    model: String,        // "RealESRGAN_x4plus" | "realesr-animevideov3" | ...
    gpu_index: Option<u32>,
) -> Result<UpscaleResult, String> {
    let _guard = crate::queue::acquire().await;
    check_memory_or_warn()?;
    let output_path = format!("{}.upscaled.mp4", video_path);
    let model_for_memory = model.clone();

    let mut args: Vec<String> = vec![
        "-i".into(), video_path.clone(),
        "-o".into(), output_path.clone(),
        "-p".into(), "realesrgan".into(),
        "-s".into(), scale.to_string(),
        "--realesrgan-model".into(), model,
    ];

    if let Some(g) = gpu_index {
        args.push("-g".into());
        args.push(g.to_string());
    }

    let bin = resolve_video2x_binary().await?;
    let mut child = nice_command(&bin)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("video2x çalıştırılamadı: {}", e))?;

    let start = Instant::now();
    let app_hb = app.clone();
    let heartbeat = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let elapsed = start.elapsed().as_secs();
            let _ = app_hb.emit(
                "amorfly://upscale-progress",
                UpscaleProgress {
                    status: format!("İşleniyor… ({} saniye geçti — bu işlem videonun kendisinden çok daha uzun sürebilir, bu normal)", elapsed),
                    elapsed_secs: elapsed,
                    done: false,
                },
            );
        }
    });

    let status = child.wait().await.map_err(|e| format!("video2x beklenirken hata: {}", e));
    heartbeat.abort();
    let status = status?;

    let _ = app.emit(
        "amorfly://upscale-progress",
        UpscaleProgress { status: "tamamlandı".into(), elapsed_secs: start.elapsed().as_secs(), done: true },
    );

    if !status.success() {
        return Err(
            "video2x işlemi başarısız oldu. Olası nedenler: Vulkan destekli GPU bulunamadı, \
             ya da video formatı desteklenmiyor. Terminalde 'video2x --list-gpus' ile GPU \
             tespitini kontrol edebilirsiniz.".to_string()
        );
    }

    crate::memory::remember(
        "islem".to_string(),
        format!("'{}' videosunu {}x büyüttü ({})", video_path, scale, model_for_memory),
    );

    Ok(UpscaleResult { output_path })
}

/// Opsiyonel: RIFE ile kare hızını (fps) artırır — çözünürlükten bağımsız
/// ayrı bir işlem. Eski, düşük fps'li (takılarak oynayan) videolar için.
#[tauri::command]
pub async fn interpolate_framerate(video_path: String, target_fps: u32) -> Result<UpscaleResult, String> {
    let output_path = format!("{}.smooth.mp4", video_path);

    let bin = resolve_video2x_binary().await?;
    let status = Command::new(&bin)
        .args([
            "-i", &video_path,
            "-o", &output_path,
            "-p", "rife",
            "--rife-fps", &target_fps.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("video2x (RIFE) çalıştırılamadı: {}", e))?;

    if !status.success() {
        return Err("Kare hızı artırma işlemi başarısız oldu.".to_string());
    }

    Ok(UpscaleResult { output_path })
}
