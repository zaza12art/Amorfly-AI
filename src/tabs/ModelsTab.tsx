import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { copyToClipboard } from '../lib/clipboard';

interface SuggestedModel {
  id: string;
  label: string;
  approx_size_gb: number;
  note: string;
}

interface ModelProgress {
  model: string;
  status: string;
  percent: number;
  done: boolean;
  error: string | null;
  completed_bytes: number;
  total_bytes: number;
  speed_bytes_per_sec: number;
  eta_seconds: number | null;
}

function formatBytes(n: number): string {
  if (n <= 0) return '0 MB';
  const mb = n / 1024 / 1024;
  return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(0)} MB`;
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec <= 0) return '';
  const mbps = (bytesPerSec * 8) / 1_000_000; // megabit/sn (internet hızı genelde böyle konuşulur)
  return mbps >= 1 ? `${mbps.toFixed(1)} Mbps` : `${(bytesPerSec / 1024).toFixed(0)} KB/sn`;
}

function formatEta(seconds: number | null): string {
  if (seconds === null || seconds <= 0) return '';
  if (seconds < 60) return `~${seconds} sn kaldı`;
  const min = Math.floor(seconds / 60);
  const sec = seconds % 60;
  return min < 60 ? `~${min} dk ${sec} sn kaldı` : `~${Math.floor(min / 60)} sa ${min % 60} dk kaldı`;
}

export default function ModelsTab() {
  const [suggested, setSuggested] = useState<SuggestedModel[]>([]);
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});

  useEffect(() => {
    invoke<SuggestedModel[]>('suggested_models').then(setSuggested);
    const unlisten = listen<ModelProgress>('amorfly://model-progress', (e) => {
      setProgress((prev) => ({ ...prev, [e.payload.model]: e.payload }));
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  async function download(id: string) {
    setProgress((prev) => ({
      ...prev,
      [id]: {
        model: id, status: 'başlıyor…', percent: 0, done: false, error: null,
        completed_bytes: 0, total_bytes: 0, speed_bytes_per_sec: 0, eta_seconds: null,
      },
    }));
    try {
      await invoke('pull_model', { model: id });
    } catch (e) {
      setProgress((prev) => ({
        ...prev,
        [id]: {
          model: id, status: 'hata', percent: 0, done: true, error: String(e),
          completed_bytes: 0, total_bytes: 0, speed_bytes_per_sec: 0, eta_seconds: null,
        },
      }));
    }
  }

  return (
    <div className="max-w-2xl">
      <p className="text-white/60 text-sm mb-4">
        Aşağıdaki öneriler gerçek <code className="bg-black/30 px-1 rounded">ollama pull</code> komutuyla indirilir —
        mock/sahte kayıt yoktur, ilerleme yüzdesi Ollama'nın kendi çıktısından okunur.
      </p>
      <div className="space-y-3">
        {suggested.map((m) => {
          const p = progress[m.id];
          return (
            <div key={m.id} className="bg-white/5 rounded-lg p-4">
              <div className="flex justify-between items-start">
                <div>
                  <h3 className="font-semibold">{m.label}</h3>
                  <p className="text-xs text-white/50">{m.note} · ~{m.approx_size_gb}GB</p>
                </div>
                <button
                  onClick={() => download(m.id)}
                  disabled={p && !p.done}
                  className="bg-[#e95420] rounded px-3 py-1.5 text-sm disabled:opacity-40 cursor-pointer"
                >
                  {p && !p.done ? 'İndiriliyor…' : 'İndir'}
                </button>
              </div>
              {p && (
                <div className="mt-2">
                  <div className="w-full bg-black/30 rounded h-2 overflow-hidden">
                    <div className="bg-[#e95420] h-2 transition-all" style={{ width: `${p.percent}%` }} />
                  </div>
                  <div className="flex items-center justify-between mt-1 text-xs text-white/40">
                    <span>
                      {p.status}
                      {p.total_bytes > 0 && ` — ${formatBytes(p.completed_bytes)} / ${formatBytes(p.total_bytes)}`}
                    </span>
                    <span className="font-medium text-white/60">{p.percent > 0 ? `%${p.percent.toFixed(0)}` : ''}</span>
                  </div>
                  {!p.done && (p.speed_bytes_per_sec > 0 || p.eta_seconds) && (
                    <div className="flex items-center justify-between mt-0.5 text-xs text-[#e95420]/80">
                      <span>{formatSpeed(p.speed_bytes_per_sec)}</span>
                      <span>{formatEta(p.eta_seconds)}</span>
                    </div>
                  )}
                  {p.error && <p className="text-xs text-red-400 mt-1">{p.error}</p>}
                </div>
              )}
              <div className="mt-2 bg-black/20 rounded p-2 flex items-center gap-2">
                <code className="flex-1 text-xs text-white/50 overflow-x-auto whitespace-nowrap">
                  ollama pull {m.id}
                </code>
                <button
                  onClick={() => copyToClipboard(`ollama pull ${m.id}`)}
                  className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-2 py-1 text-xs cursor-pointer"
                  title="Otomatik indirme çalışmazsa terminale yapıştır"
                >
                  Kopyala
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
