import { invoke } from '@tauri-apps/api/core';

/**
 * Bir hatayı hem ekranda göstermek hem de ~/.local/share/amorfly/logs
 * altındaki dosyaya kaydetmek için kullanılır. Kullanıcı "çalışmıyor"
 * dediğinde Tanılama sekmesinden bu logları görebilirsin.
 */
export async function logError(context: string, error: unknown): Promise<string> {
  const message = String(error);
  try {
    await invoke('log_frontend_error', { message: `${context}: ${message}` });
  } catch {
    // loglama başarısız olsa bile kullanıcıya hatayı göstermeye devam et
  }
  return message;
}
