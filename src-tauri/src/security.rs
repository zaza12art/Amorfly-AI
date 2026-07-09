// Amorfly AI — Gerçek Şifreleme Katmanı
// Eski `encryption.ts` sadece Base64 + ters çevirmeydi (şifreleme DEĞİLDİ).
// Bu modül gerçek AES-256-GCM kullanır. Anahtar, kullanıcının ev dizininde
// ~/.config/amorfly/vault/amorfly.key altında 0600 izniyle saklanır ve
// diskten okunan/yazılan hiçbir veri düz metin olarak durmaz.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn vault_dir() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("Amorfly AI");
    dir.push("vault");
    dir
}

fn key_path() -> PathBuf {
    let mut p = vault_dir();
    p.push("amorfly.key");
    p
}

/// Anahtar yoksa oluşturur (32 byte / 256 bit), varsa okur.
/// Dosya izinleri Linux'ta 0600 (sadece sahibi okur/yazar) olarak zorlanır.
fn get_or_create_key() -> Result<[u8; 32], String> {
    let dir = vault_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Vault dizini oluşturulamadı: {}", e))?;

    let path = key_path();
    if path.exists() {
        let data = fs::read(&path).map_err(|e| format!("Anahtar okunamadı: {}", e))?;
        if data.len() != 32 {
            return Err("Anahtar dosyası bozuk (beklenen 32 byte değil).".to_string());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data);
        Ok(key)
    } else {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        let mut file = fs::File::create(&path).map_err(|e| format!("Anahtar yazılamadı: {}", e))?;
        file.write_all(&key).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, perms).map_err(|e| format!("İzin ayarlanamadı: {}", e))?;
        }

        Ok(key)
    }
}

/// Metni AES-256-GCM ile şifreler. Çıktı: base64(nonce || ciphertext)
#[tauri::command]
pub fn encrypt_string(plaintext: String) -> Result<String, String> {
    let key = get_or_create_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Şifreleme hatası: {}", e))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(combined))
}

/// `encrypt_string` çıktısını geri çözer.
#[tauri::command]
pub fn decrypt_string(payload: String) -> Result<String, String> {
    let key = get_or_create_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let combined = STANDARD
        .decode(payload)
        .map_err(|e| format!("Base64 çözümlenemedi: {}", e))?;
    if combined.len() < 12 {
        return Err("Geçersiz şifreli veri.".to_string());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Çözme başarısız — anahtar uyuşmuyor ya da veri bozuk.".to_string())?;

    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

/// Şifrelenmiş bir değeri diskte belirli bir dosyada saklar (ör. ayarlar,
/// kullanım geçmişi). Dosya adı basit bir anahtar (ör. "settings", "habits").
#[tauri::command]
pub fn vault_write(name: String, plaintext: String) -> Result<(), String> {
    let encrypted = encrypt_string(plaintext)?;
    let mut path = vault_dir();
    path.push(format!("{}.enc", name));
    fs::write(&path, encrypted).map_err(|e| format!("Vault yazma hatası: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[tauri::command]
pub fn vault_read(name: String) -> Result<Option<String>, String> {
    let mut path = vault_dir();
    path.push(format!("{}.enc", name));
    if !path.exists() {
        return Ok(None);
    }
    let encrypted = fs::read_to_string(&path).map_err(|e| format!("Vault okuma hatası: {}", e))?;
    Ok(Some(decrypt_string(encrypted)?))
}
