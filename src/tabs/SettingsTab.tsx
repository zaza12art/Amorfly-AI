import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrlWithSystem } from '../lib/openFile';
import { LANGUAGES } from '../lib/languages';

interface OnlineProviderConfig {
  label: string;
  base_url: string;
  api_key: string;
  model: string;
}

interface VoiceSettings {
  whisperBin: string;
  whisperModelPath: string;
  piperBin: string;
  piperVoiceModel: string;
  responseLanguage: string;
  autoRefine: boolean;
  autoSpeak: boolean;
  recordSeconds: number;
}

const DEFAULT_VOICE: VoiceSettings = {
  whisperBin: 'whisper-cli',
  whisperModelPath: '',
  piperBin: 'piper',
  piperVoiceModel: 'tr_TR-dfki-medium',
  responseLanguage: 'Türkçe',
  autoRefine: true,
  autoSpeak: false,
  recordSeconds: 6,
};

const GROQ_PRESET = {
  label: 'Groq (ücretsiz, kayıtsız değil ama kredi kartı istemiyor)',
  base_url: 'https://api.groq.com/openai/v1',
  model: 'llama-3.3-70b-versatile',
};

export default function SettingsTab() {
  const [cfg, setCfg] = useState<OnlineProviderConfig>({ label: '', base_url: '', api_key: '', model: '' });
  const [saved, setSaved] = useState(false);
  const [habitsEnabled, setHabitsEnabled] = useState(localStorage.getItem('habitsEnabled') !== 'false');
  const [voice, setVoice] = useState<VoiceSettings>(DEFAULT_VOICE);
  const [voiceSaved, setVoiceSaved] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [memoryEntries, setMemoryEntries] = useState<{ timestamp: string; category: string; text: string }[]>([]);
  const [showMemory, setShowMemory] = useState(false);

  interface RouterConfig {
    genel: string; kod: string; belge_analiz: string; excel: string; dil_egitimi: string; gorsel_analiz: string;
  }
  const [routerConfig, setRouterConfig] = useState<RouterConfig | null>(null);
  const [installedModels, setInstalledModels] = useState<string[]>([]);
  const [routerSaved, setRouterSaved] = useState(false);

  useEffect(() => {
    invoke<RouterConfig>('get_router_config').then(setRouterConfig).catch(() => {});
    invoke<string[]>('list_ollama_models').then(setInstalledModels).catch(() => {});
  }, []);

  async function saveRouterConfig() {
    if (!routerConfig) return;
    await invoke('save_router_config', { config: routerConfig });
    setRouterSaved(true);
    setTimeout(() => setRouterSaved(false), 1500);
  }

  useEffect(() => {
    invoke<string>('get_app_version').then(setAppVersion).catch(() => {});
  }, []);

  async function loadMemory() {
    const entries = await invoke<{ timestamp: string; category: string; text: string }[]>('recall_all');
    setMemoryEntries(entries);
    setShowMemory(true);
  }

  async function clearMemory() {
    const ok = window.confirm('Tüm hafıza (geçmiş işlem kayıtları ve tercihler) silinecek. Emin misin?');
    if (!ok) return;
    await invoke('clear_memory');
    setMemoryEntries([]);
  }

  useEffect(() => {
    invoke<OnlineProviderConfig | null>('get_online_provider').then((c) => {
      if (c) setCfg(c);
    });
    invoke<string | null>('vault_read', { name: 'voice_settings' }).then((raw) => {
      if (raw) setVoice({ ...DEFAULT_VOICE, ...JSON.parse(raw) });
    }).catch(() => {});
  }, []);

  async function saveVoice() {
    await invoke('vault_write', { name: 'voice_settings', plaintext: JSON.stringify(voice) });
    setVoiceSaved(true);
    setTimeout(() => setVoiceSaved(false), 1500);
  }

  async function save() {
    await invoke('save_online_provider', { config: cfg });
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  async function clear() {
    await invoke('clear_online_provider');
    setCfg({ label: '', base_url: '', api_key: '', model: '' });
  }

  function useGroqPreset() {
    setCfg((prev) => ({ ...GROQ_PRESET, api_key: prev.api_key }));
  }

  function toggleHabits(v: boolean) {
    setHabitsEnabled(v);
    localStorage.setItem('habitsEnabled', String(v));
  }

  const [habitLog, setHabitLog] = useState<{ totals_seconds: Record<string, number>; last_active_app: string } | null>(null);
  const [showHabits, setShowHabits] = useState(false);

  async function loadHabitLog() {
    try {
      const log = await invoke<{ totals_seconds: Record<string, number>; last_active_app: string }>('get_habit_log');
      setHabitLog(log);
      setShowHabits(true);
    } catch {
      setHabitLog(null);
      setShowHabits(true);
    }
  }

  return (
    <div className="max-w-2xl space-y-6">
      <section className="bg-white/5 rounded-lg p-4">
        <h2 className="font-semibold mb-1">Online Sağlayıcı (opsiyonel)</h2>
        <p className="text-xs text-white/50 mb-3">
          Google/Gemini'ye hiçbir bağlılık yok. Buraya OpenAI-uyumlu herhangi bir endpoint
          girebilirsin (ör. Groq, OpenRouter, kendi sunucun). Boş bırakırsan uygulama tamamen
          yerel (Ollama) çalışmaya devam eder — bu tamamen opsiyonel bir ek.
        </p>

        <div className="bg-[#e95420]/10 border border-[#e95420]/30 rounded-lg p-3 mb-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm font-medium">Önerilen: Groq</p>
              <p className="text-xs text-white/50">
                Gerçekten ücretsiz, kredi kartı istemiyor. Uygulama içinden "oturum açma" yapmıyoruz
                (bu güvensiz olurdu) — buton sistem tarayıcında Groq'un anahtar sayfasını açar,
                orada Google/GitHub ile tek tıkla giriş yapıp anahtarı buraya yapıştırırsın.
              </p>
            </div>
            <div className="shrink-0 flex flex-col gap-1">
              <button onClick={() => openUrlWithSystem('https://console.groq.com/keys')} className="bg-[#e95420] rounded px-3 py-1.5 text-xs whitespace-nowrap cursor-pointer">
                Groq Sayfasını Aç
              </button>
              <button onClick={useGroqPreset} className="bg-white/10 rounded px-3 py-1.5 text-xs whitespace-nowrap cursor-pointer">
                Alanları Doldur
              </button>
            </div>
          </div>
        </div>

        <div className="space-y-2 text-sm">
          <input placeholder="Etiket (ör. Groq)" value={cfg.label} onChange={(e) => setCfg({ ...cfg, label: e.target.value })} className="w-full bg-white/10 text-white rounded px-2 py-1" />
          <input placeholder="Base URL (ör. https://api.groq.com/openai/v1)" value={cfg.base_url} onChange={(e) => setCfg({ ...cfg, base_url: e.target.value })} className="w-full bg-white/10 text-white rounded px-2 py-1" />
          <input placeholder="API Anahtarı (console.groq.com'dan alınır)" type="password" value={cfg.api_key} onChange={(e) => setCfg({ ...cfg, api_key: e.target.value })} className="w-full bg-white/10 text-white rounded px-2 py-1" />
          <input placeholder="Model adı (ör. llama-3.3-70b-versatile)" value={cfg.model} onChange={(e) => setCfg({ ...cfg, model: e.target.value })} className="w-full bg-white/10 text-white rounded px-2 py-1" />
        </div>
        <div className="flex gap-2 mt-3">
          <button onClick={save} className="bg-[#e95420] rounded px-4 py-1.5 text-sm cursor-pointer">{saved ? 'Kaydedildi ✓' : 'Kaydet (şifreli)'}</button>
          <button onClick={clear} className="bg-white/10 rounded px-4 py-1.5 text-sm cursor-pointer">Temizle</button>
        </div>
        <p className="text-xs text-white/30 mt-2">
          API anahtarı diskte AES-256-GCM ile şifreli tutulur (~/.config/Amorfly AI/vault).
        </p>
      </section>

      <section className="bg-white/5 rounded-lg p-4">
        <h2 className="font-semibold mb-1">Ses & Dil</h2>
        <p className="text-xs text-white/50 mb-3">
          Sesle soru sorma, cevapları sesli okuma ve devrik/saçma cümleleri önlemek için
          otomatik dil düzeltmesi. Sohbet sekmesi bu ayarları kullanır.
        </p>

        <div className="space-y-2 text-sm mb-3">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <label className="flex flex-col gap-1">
              <span className="text-white/50 text-xs">whisper.cpp binary</span>
              <input value={voice.whisperBin} onChange={(e) => setVoice({ ...voice, whisperBin: e.target.value })} className="bg-white/10 text-white rounded px-2 py-1" />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-white/50 text-xs">whisper model yolu (.bin)</span>
              <input value={voice.whisperModelPath} onChange={(e) => setVoice({ ...voice, whisperModelPath: e.target.value })} placeholder="/home/kullanici/whisper/ggml-small.bin" className="bg-white/10 text-white rounded px-2 py-1" />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-white/50 text-xs">piper binary</span>
              <input value={voice.piperBin} onChange={(e) => setVoice({ ...voice, piperBin: e.target.value })} className="bg-white/10 text-white rounded px-2 py-1" />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-white/50 text-xs">piper ses modeli</span>
              <div className="flex gap-1">
                <input value={voice.piperVoiceModel} onChange={(e) => setVoice({ ...voice, piperVoiceModel: e.target.value })} className="bg-white/10 text-white rounded px-2 py-1 flex-1" />
                <button
                  onClick={async () => {
                    const path = await invoke<string>('download_piper_turkish_voice');
                    setVoice((v) => ({ ...v, piperVoiceModel: path }));
                  }}
                  className="bg-white/10 rounded px-2 text-xs whitespace-nowrap cursor-pointer"
                  title="Türkçe ses modelini Hugging Face'ten indir (kayıt gerekmez)"
                >
                  İndir
                </button>
              </div>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-white/50 text-xs">Yanıt dili</span>
              <select value={voice.responseLanguage} onChange={(e) => setVoice({ ...voice, responseLanguage: e.target.value })} className="bg-white/10 text-white rounded px-2 py-1 cursor-pointer">
                {LANGUAGES.map((l) => <option key={l.whisper} value={l.label}>{l.label}</option>)}
              </select>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-white/50 text-xs">Kayıt süresi (mikrofon)</span>
              <select value={voice.recordSeconds} onChange={(e) => setVoice({ ...voice, recordSeconds: Number(e.target.value) })} className="bg-white/10 text-white rounded px-2 py-1">
                <option value={4}>4 saniye</option>
                <option value={6}>6 saniye</option>
                <option value={10}>10 saniye</option>
                <option value={15}>15 saniye</option>
              </select>
            </label>
          </div>
        </div>

        <div className="space-y-1 mb-3">
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={voice.autoRefine} onChange={(e) => setVoice({ ...voice, autoRefine: e.target.checked })} />
            Cevapları otomatik dil düzeltmesinden geçir (devrik/saçma cümleleri önler, ekstra bir model çağrısı gerektirdiği için biraz yavaşlatır)
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={voice.autoSpeak} onChange={(e) => setVoice({ ...voice, autoSpeak: e.target.checked })} />
            Her cevabı otomatik sesli oku
          </label>
        </div>

        <button onClick={saveVoice} className="bg-[#e95420] rounded px-4 py-1.5 text-sm cursor-pointer">
          {voiceSaved ? 'Kaydedildi ✓' : 'Ses & Dil Ayarlarını Kaydet'}
        </button>

        <div className="text-xs text-white/40 bg-white/5 rounded p-3 mt-3">
          Gerekli kurulumlar: ffmpeg (mikrofon kaydı ve seslendirme çalma) · whisper.cpp
          (sesle tanıma) · Piper TTS + ses modeli (sesli okuma). Mikrofon PulseAudio/PipeWire
          üzerinden okunur — çoğu Ubuntu türevinde varsayılan olarak zaten kurulu.
        </div>
      </section>

      <section className="bg-white/5 rounded-lg p-4">
        <h2 className="font-semibold mb-1">Kullanım Alışkanlığı Takibi</h2>
        <p className="text-xs text-white/50 mb-3">
          Yalnızca X11'de çalışır (xdotool gerekir). Hiçbir veri ağa gönderilmez, yalnızca
          şifreli olarak yerelde tutulur. İstediğin an kapatabilirsin.
        </p>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={habitsEnabled} onChange={(e) => toggleHabits(e.target.checked)} />
          Alışkanlık takibini ve öneri bildirimlerini etkinleştir
        </label>
        <button onClick={loadHabitLog} className="mt-3 bg-white/10 rounded px-3 py-1.5 text-sm cursor-pointer">
          Şu Ana Kadar Öğrenileni Göster
        </button>
        {showHabits && (
          <div className="mt-3 bg-black/30 rounded p-3 text-xs">
            {!habitLog || Object.keys(habitLog.totals_seconds).length === 0 ? (
              <p className="text-white/30">
                Henüz veri yok — ya alışkanlık takibi kapalı, ya X11 dışında bir görüntü sunucusu
                (Wayland) kullanıyorsun, ya da uygulama az önce açıldı ve henüz bir "tik" atmadı
                (ilk kayıt için ~60 saniye gerekir).
              </p>
            ) : (
              <div className="space-y-1">
                <p className="text-white/50 mb-2">
                  Şu an aktif: <span className="text-white/80">{habitLog.last_active_app}</span>
                </p>
                {Object.entries(habitLog.totals_seconds)
                  .sort((a, b) => b[1] - a[1])
                  .slice(0, 8)
                  .map(([app, secs]) => (
                    <div key={app} className="flex justify-between text-white/60">
                      <span className="truncate mr-2">{app || '(bilinmeyen pencere)'}</span>
                      <span className="shrink-0">{Math.round(secs / 60)} dk</span>
                    </div>
                  ))}
              </div>
            )}
          </div>
        )}
      </section>

      <section className="bg-white/5 rounded-lg p-4">
        <h2 className="font-semibold mb-1">Hafıza</h2>
        <p className="text-xs text-white/50 mb-3">
          Amorfly, belge analizi, altyazı üretimi, video kalite artırma gibi işlemleri otomatik
          olarak yerel bir hafızada tutar ve sohbet ederken bunu kısa bir özet olarak modele
          verir — böylece daha önce ne yaptığını "hatırlıyormuş" gibi davranır. Tamamen yerel,
          şifreli, ve senin kontrolünde.
        </p>
        <div className="flex gap-2">
          <button onClick={loadMemory} className="bg-white/10 rounded px-3 py-1.5 text-sm cursor-pointer">Hafızayı Görüntüle</button>
          <button onClick={clearMemory} className="bg-red-500/20 text-red-400 rounded px-3 py-1.5 text-sm cursor-pointer">Hafızayı Temizle</button>
        </div>
        {showMemory && (
          <div className="mt-3 bg-black/30 rounded p-3 max-h-56 overflow-y-auto text-xs space-y-1">
            {memoryEntries.length === 0 && <p className="text-white/30">Henüz hafıza kaydı yok.</p>}
            {[...memoryEntries].reverse().map((m, i) => (
              <p key={i} className="text-white/50"><span className="text-white/30">[{m.timestamp}]</span> {m.text}</p>
            ))}
          </div>
        )}
      </section>

      <section className="bg-white/5 rounded-lg p-4">
        <h2 className="font-semibold mb-1">AI Router — Görev Türüne Göre Model</h2>
        <p className="text-xs text-white/50 mb-3">
          Her görev aynı modelle aynı kalitede yapılmaz — örneğin kod sorularında kod-odaklı bir
          model (qwen2.5-coder gibi) genel bir modelden çok daha tutarlı sonuç verir. Burada hangi
          görevde hangi kurulu modelin kullanılacağını belirleyebilirsin (Excel Üretici, Belge
          Analizi ve Dil Eğitimi bu ayarı otomatik kullanır).
        </p>
        {installedModels.length === 0 ? (
          <p className="text-xs text-white/30">Henüz kurulu model yok — önce Modeller sekmesinden bir şeyler indir.</p>
        ) : routerConfig && (
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
            {([
              ['genel', 'Genel sohbet'],
              ['kod', 'Kod üretimi'],
              ['belge_analiz', 'Belge analizi'],
              ['excel', 'Excel üretici'],
              ['dil_egitimi', 'Dil eğitimi'],
              ['gorsel_analiz', 'Görsel analiz'],
            ] as [keyof RouterConfig, string][]).map(([key, label]) => (
              <label key={key} className="flex flex-col gap-1">
                <span className="text-white/50 text-xs">{label}</span>
                <select
                  value={routerConfig[key]}
                  onChange={(e) => setRouterConfig({ ...routerConfig, [key]: e.target.value })}
                  className="bg-white/10 text-white rounded px-2 py-1.5 cursor-pointer"
                >
                  {/* Ollama modelleri genelde ":latest" etiketiyle listelenir (ör.
                      "llama3.2:latest") — etiketi göz ardı ederek karşılaştırıyoruz,
                      yoksa kurulu bir model burada "kurulu değil" gibi görünebilirdi. */}
                  {!installedModels.some((m) => m.split(':')[0] === routerConfig[key].split(':')[0]) && (
                    <option value={routerConfig[key]}>{routerConfig[key]} (henüz kurulu değil)</option>
                  )}
                  {installedModels.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              </label>
            ))}
          </div>
        )}
        <button onClick={saveRouterConfig} className="mt-3 bg-[#e95420] rounded px-3 py-1.5 text-sm cursor-pointer">
          {routerSaved ? 'Kaydedildi ✓' : 'Kaydet'}
        </button>
      </section>

      <section className="bg-white/5 rounded-lg p-4">
        <h2 className="font-semibold mb-1">Hakkında</h2>
        <p className="text-sm text-white/60">Amorfly AI — sürüm {appVersion || '…'}</p>
        <p className="text-xs text-white/30 mt-1">
          Hangi modüllerin/araçların kurulu ve aktif olduğunu görmek için "Tanılama" sekmesine bak.
        </p>
      </section>
    </div>
  );
}
