import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { RefreshCw, Terminal, CheckCircle2, XCircle, FileClock } from 'lucide-react';
import { copyToClipboard } from '../lib/clipboard';

interface CheckResult {
  name: string;
  ok: boolean;
  detail: string;
  auto_installable: boolean;
  apt_package: string | null;
}

interface DiagnosticsReport {
  checks: CheckResult[];
  app_version: string;
}

const AUTO_INSTALL_COMMAND: Record<string, string> = {
  'Ollama': 'install_ollama_portable',
  'Video2X': 'install_video2x_portable',
  'Piper TTS': 'install_piper_portable',
};

// Her modülün hangi kontrollere bağlı olduğu — özet rozetler için.
const MODULES: { name: string; requires: string[] }[] = [
  { name: 'Sohbet', requires: ['Ollama'] },
  { name: 'Altyazı / Dublaj', requires: ['ffmpeg', 'whisper.cpp (whisper-cli)'] },
  { name: 'Kalite Artırma', requires: ['Video2X'] },
  { name: 'Belge & Görsel', requires: ['pandoc', 'poppler-utils (pdftotext)', 'LibreOffice'] },
  { name: 'Ses & Dil', requires: ['ffmpeg', 'whisper.cpp (whisper-cli)', 'Piper TTS'] },
];

export default function DiagnosticsTab() {
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [scanning, setScanning] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [logPath, setLogPath] = useState('');

  useEffect(() => {
    scan();
    invoke<string>('get_log_file_path').then(setLogPath).catch(() => {});
  }, []);

  async function scan() {
    setScanning(true);
    try {
      const r = await invoke<DiagnosticsReport>('run_diagnostics');
      setReport(r);
    } finally {
      setScanning(false);
    }
  }

  async function autoInstall(name: string) {
    const cmd = AUTO_INSTALL_COMMAND[name];
    if (!cmd) return;
    setInstalling(name);
    try {
      const result = await invoke<{ installed_path: string }>(cmd);

      // Piper taşınabilir olarak kurulduğunda PATH'te değildir — tam yolunu
      // Ses & Dil ayarlarına otomatik yazıyoruz ki Sohbet/Altyazı sekmeleri
      // "piper bulunamadı" hatası vermesin.
      if (name === 'Piper TTS' && result?.installed_path) {
        try {
          const raw = await invoke<string | null>('vault_read', { name: 'voice_settings' });
          const current = raw ? JSON.parse(raw) : {};
          current.piperBin = result.installed_path;
          await invoke('vault_write', { name: 'voice_settings', plaintext: JSON.stringify(current) });
        } catch {
          // ayar güncellenemedi, kullanıcı Ayarlar'dan elle girebilir
        }
      }

      await scan();
    } catch {
      await scan();
    } finally {
      setInstalling(null);
    }
  }

  async function terminalInstall(pkg: string) {
    const ok = window.confirm(
      `Bir terminal açılıp şu komut çalıştırılacak:\n\nsudo apt install -y ${pkg}\n\n` +
      `Şifreni kendi açtığın terminale gireceksin, uygulama şifreni görmez/tutmaz. Devam edilsin mi?`
    );
    if (!ok) return;
    await invoke('open_terminal_install', { aptPackages: pkg.split(' '), confirmed: true });
  }

  async function loadLogs() {
    const l = await invoke<string[]>('get_recent_logs', { lines: 80 });
    setLogs(l);
    setShowLogs(true);
  }

  function checkFor(name: string) {
    return report?.checks.find((c) => c.name === name);
  }

  function moduleActive(requires: string[]) {
    return requires.every((r) => checkFor(r)?.ok);
  }

  return (
    <div className="max-w-3xl space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="font-semibold">Tanılama</h2>
          <p className="text-xs text-white/40">Amorfly AI v{report?.app_version ?? '…'}</p>
        </div>
        <button onClick={scan} disabled={scanning} className="flex items-center gap-2 bg-white/10 rounded px-3 py-1.5 text-sm disabled:opacity-40 cursor-pointer">
          <RefreshCw size={14} className={scanning ? 'animate-spin' : ''} /> Yeniden Tara
        </button>
      </div>

      <div className="bg-white/5 rounded-lg p-3 text-xs text-white/50">
        <p className="mb-2">
          Bir motorun "Otomatik Kur" butonu çalışmazsa (ağ/link sorunu vb.), aşağıdaki komutu
          terminale yapıştır — aynı motorları (Ollama, Piper, Video2X, Türkçe ses modeli) Amorfly
          AI'ın arayacağı tam klasöre indirir. Uygulamayı kapat, tekrar aç, burada otomatik yeşil görünür.
        </p>
        <div className="flex items-center gap-2">
          <code className="flex-1 bg-black/30 rounded px-2 py-1 overflow-x-auto whitespace-nowrap">
            curl -fsSL https://raw.githubusercontent.com/zaza12art/Amorfly-AI/main/motorlari_elle_kur.sh | bash
          </code>
          <button
            onClick={() => copyToClipboard('curl -fsSL https://raw.githubusercontent.com/zaza12art/Amorfly-AI/main/motorlari_elle_kur.sh | bash')}
            className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-2 py-1 cursor-pointer"
          >
            Kopyala
          </button>
        </div>
      </div>

      {/* Modül özeti */}
      <div className="flex flex-wrap gap-2">
        {MODULES.map((m) => {
          const active = report ? moduleActive(m.requires) : false;
          return (
            <span
              key={m.name}
              className={'text-xs rounded-full px-3 py-1 ' + (active ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400')}
            >
              {active ? '● ' : '○ '}{m.name}
            </span>
          );
        })}
      </div>

      {/* Detaylı kontrol listesi */}
      <div className="bg-white/5 rounded-lg divide-y divide-white/10">
        {report?.checks.map((c) => (
          <div key={c.name} className="flex items-center justify-between gap-3 p-3">
            <div className="flex items-start gap-2 min-w-0">
              {c.ok ? <CheckCircle2 size={16} className="text-green-400 shrink-0 mt-0.5" /> : <XCircle size={16} className="text-red-400 shrink-0 mt-0.5" />}
              <div className="min-w-0">
                <p className="text-sm">{c.name}</p>
                <p className="text-xs text-white/40">{c.detail}</p>
              </div>
            </div>
            {!c.ok && c.auto_installable && (
              <button
                onClick={() => autoInstall(c.name)}
                disabled={installing === c.name}
                className="shrink-0 bg-[#e95420] rounded px-3 py-1.5 text-xs disabled:opacity-40 cursor-pointer"
              >
                {installing === c.name ? 'Kuruluyor…' : 'Otomatik Kur'}
              </button>
            )}
            {!c.ok && !c.auto_installable && c.apt_package && (
              <button
                onClick={() => terminalInstall(c.apt_package!)}
                className="shrink-0 flex items-center gap-1 bg-white/10 rounded px-3 py-1.5 text-xs whitespace-nowrap"
              >
                <Terminal size={12} /> Terminalde Kur
              </button>
            )}
          </div>
        ))}
        {!report && <p className="p-4 text-white/40 text-sm">Taranıyor…</p>}
      </div>

      {/* Log görüntüleyici */}
      <div className="bg-white/5 rounded-lg p-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium flex items-center gap-2"><FileClock size={14} /> Log Kayıtları</h3>
            <p className="text-xs text-white/30 mt-1 break-all">{logPath}</p>
          </div>
          <button onClick={loadLogs} className="bg-white/10 rounded px-3 py-1.5 text-xs cursor-pointer">Son 80 Satırı Göster</button>
        </div>
        {showLogs && (
          <pre className="mt-3 bg-black/40 rounded p-3 text-xs text-white/60 max-h-64 overflow-y-auto whitespace-pre-wrap">
            {logs.length > 0 ? logs.join('\n') : 'Henüz log kaydı yok.'}
          </pre>
        )}
        <p className="text-xs text-white/30 mt-2">
          Bir şey çalışmadığında buradaki loglar (ve sekmelerde çıkan hata mesajları) nedeni gösterir —
          bir modülün çökmesi diğerlerini ya da uygulamayı etkilemez.
        </p>
      </div>
    </div>
  );
}
