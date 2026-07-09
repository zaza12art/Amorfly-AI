// Amorfly AI — Sistemde Dosya/URL Açma (çift katmanlı, sorunsuz)
//
// Tauri v2'de dosya/URL açma işlevi eskiden @tauri-apps/plugin-shell'in
// open() fonksiyonundaydı, artık ayrı bir "Opener" eklentisine taşındı.
// Eski fonksiyon hâlâ çalışıyor ama "kullanımdan kalkıyor" olarak
// işaretli. Burada ÖNCE yeni Opener'ı deniyoruz, o başarısız olursa
// (ör. henüz güncellenmemiş bir ortamda) eski Shell yöntemine düşüyoruz
// — böylece hangisi "aktif" olursa olsun kullanıcı takılmıyor.

import { openPath, openUrl as openerOpenUrl } from '@tauri-apps/plugin-opener';
import { open as shellOpen } from '@tauri-apps/plugin-shell';

/** Bir dosyayı sistemin varsayılan uygulamasıyla açar (video oynatıcı, vb.) */
export async function openFileWithSystem(path: string): Promise<void> {
  try {
    await openPath(path);
    return;
  } catch {
    // Yeni Opener eklentisi başarısız oldu — eski Shell yöntemine düş.
  }
  await shellOpen(path);
}

/** Bir URL'yi sistemin varsayılan tarayıcısıyla açar. */
export async function openUrlWithSystem(url: string): Promise<void> {
  try {
    await openerOpenUrl(url);
    return;
  } catch {
    // Yeni Opener eklentisi başarısız oldu — eski Shell yöntemine düş.
  }
  await shellOpen(url);
}
