// Amorfly AI — Görev Kuyruğu (Queue Engine)
//
// Amaç: aynı anda birden fazla ağır istek gelirse (ör. hem sohbet hem
// altyazı üretimi hem kalite artırma aynı anda başlatılırsa) bunlar
// birbirine karışmasın, sırayla işlensin. Global, tek bir kilit —
// aşırı mühendislik yapmadan gerçek sorunu çözer: "5 görev aynı anda
// gelirse karışabilir" şikayetinin doğrudan cevabı budur.
//
// Fonksiyon imzalarını değiştirmemek için Tauri'nin State enjeksiyonu
// yerine process-wide bir static kullanılıyor — böylece hem Tauri
// komutlarından hem agent.rs'in doğrudan çağırdığı fonksiyonlardan
// aynı şekilde çağrılabiliyor.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};

static QUEUE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static WAITING: AtomicUsize = AtomicUsize::new(0);

fn lock() -> &'static Mutex<()> {
    QUEUE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Ağır bir işlemin (Ollama sohbeti, altyazı/dublaj üretimi, kalite
/// artırma, belge/görsel analizi, Excel üretimi) başında çağrılır.
/// Dönen guard, fonksiyon bitene kadar (scope sonuna kadar) canlı
/// tutulmalı — böylece bir sonraki bekleyen iş ancak bu bitince başlar.
pub async fn acquire() -> MutexGuard<'static, ()> {
    WAITING.fetch_add(1, Ordering::SeqCst);
    let guard = lock().lock().await;
    WAITING.fetch_sub(1, Ordering::SeqCst);
    guard
}

/// Şu anda kilidi bekleyen (henüz sırası gelmemiş) iş sayısı.
pub fn waiting_count() -> usize {
    WAITING.load(Ordering::SeqCst)
}

/// Frontend'in "şu an X görev sırada bekliyor" gibi bir bilgi
/// gösterebilmesi için.
#[tauri::command]
pub fn queue_status() -> usize {
    waiting_count()
}
