import { useEffect, useState, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import ChatTab from './tabs/ChatTab';
import LanguageLearningTab from './tabs/LanguageLearningTab';
import ModelsTab from './tabs/ModelsTab';
import SubtitlesTab from './tabs/SubtitlesTab';
import UpscaleTab from './tabs/UpscaleTab';
import DocumentsTab from './tabs/DocumentsTab';
import DiagnosticsTab from './tabs/DiagnosticsTab';
import TasksTab from './tabs/TasksTab';
import SettingsTab from './tabs/SettingsTab';
import { ChevronLeft, Lock, Cloud, Sparkles } from 'lucide-react';

/** ÖNEMLİ: Logo artık ayrı bir .png dosyası DEĞİL, doğrudan kodun içine
 * gömülü bir SVG. Sebep: harici görsel dosyaları git'e manuel taşıma
 * sürecinde (zip -> Codespace -> commit) tekrar tekrar kayboluyordu ve
 * bu, "Cannot find module" tip hatalarına yol açıyordu. SVG kod içinde
 * olduğu için ARTIK KAYBOLMASI MÜMKÜN DEĞİL — her zaman diğer kodla
 * birlikte, aynı dosyada senkron kalır. */
function LogoOctagon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 100 100" className={className} xmlns="http://www.w3.org/2000/svg">
      <polygon
        points="30,5 70,5 95,30 95,70 70,95 30,95 5,70 5,30"
        fill="#3a3836"
      />
      <polygon
        points="50,18 74,34 82,58 68,80 32,80 18,58 26,34"
        fill="#8b5cf6"
      />
      <polygon
        points="50,18 74,34 50,50"
        fill="#a78bfa"
      />
      <polygon
        points="50,18 26,34 50,50"
        fill="#7c3aed"
      />
    </svg>
  );
}

type Tab = 'chat' | 'language' | 'models' | 'subtitles' | 'upscale' | 'documents' | 'tasks' | 'diagnostics' | 'settings';
export type WorkMode = 'lokal' | 'hibrit' | 'online';

const WORK_MODE_KEY = 'work_mode';

const MODE_INFO: { id: WorkMode; label: string; desc: string; icon: ReactNode }[] = [
  {
    id: 'lokal',
    label: 'Lokal',
    desc: 'Sadece Amorfly içindeki yerel motorlar (Ollama). Hiçbir şey bilgisayarından çıkmaz — tamamen gizli.',
    icon: <Lock size={16} />,
  },
  {
    id: 'hibrit',
    label: 'Hibrit',
    desc: 'Önce yerel motor bir taslak üretir, sonra kayıtlı online sağlayıcı (Groq) bu taslağı gözden geçirip son hâlini verir. Taslak + isteğin dışarı çıktığını unutma.',
    icon: <Sparkles size={16} />,
  },
  {
    id: 'online',
    label: 'Online',
    desc: 'Doğrudan kayıtlı online sağlayıcı (Groq) cevap verir, yerel motor devreye girmez. En hızlı, gizlilik önceliği yok.',
    icon: <Cloud size={16} />,
  },
];

export default function App() {
  const [tab, setTab] = useState<Tab>('chat');
  // Sol çekmece (sohbet geçmişi) durumu artık burada tutuluyor ki üst
  // başlıktaki logo, hangi sekmede olursa olsun onu aç/kapa yapabilsin.
  const [sidebarOpen, setSidebarOpen] = useState(true);

  // Lokal / Hibrit / Online çalışma modu — sağ üstteki çekmeceden seçilir,
  // şifreli vault'ta saklanır (kapatıp açınca hatırlanır). Sohbet, Dil
  // Eğitimi, Belge Analizi ve Excel Üretici bu tek moda göre davranır.
  const [workMode, setWorkMode] = useState<WorkMode>('lokal');
  const [modeDrawerOpen, setModeDrawerOpen] = useState(false);

  useEffect(() => {
    invoke<string | null>('vault_read', { name: WORK_MODE_KEY })
      .then((raw) => {
        if (raw && (raw === 'lokal' || raw === 'hibrit' || raw === 'online')) {
          setWorkMode(raw as WorkMode);
        }
      })
      .catch(() => {});
  }, []);

  function selectWorkMode(mode: WorkMode) {
    setWorkMode(mode);
    invoke('vault_write', { name: WORK_MODE_KEY, plaintext: mode }).catch(() => {});
  }

  // Faz 4: kullanım alışkanlığı takibi — 60 saniyede bir "tik" atar,
  // eşik aşılırsa gerçek bir masaüstü bildirimi gösterir. Tamamen yerel,
  // kapatılabilir (bkz. Ayarlar sekmesi -> localStorage 'habitsEnabled').
  useEffect(() => {
    const enabled = localStorage.getItem('habitsEnabled') !== 'false';
    if (!enabled) return;

    const interval = setInterval(async () => {
      try {
        await invoke('record_activity_tick', { intervalSeconds: 60 });
        const suggestion = await invoke<string | null>('suggest_from_habits');
        if (suggestion) {
          await invoke('show_suggestion_notification', { message: suggestion });
        }
      } catch {
        // X11 yoksa (Wayland) ya da xdotool kurulu değilse burada sessizce
        // geçiyoruz — Ayarlar sekmesinde durum ayrıca gösteriliyor.
      }
    }, 60_000);

    return () => clearInterval(interval);
  }, []);

  const tabs: { id: Tab; label: string }[] = [
    { id: 'chat', label: 'Sohbet' },
    { id: 'language', label: 'Dil Eğitimi' },
    { id: 'models', label: 'Modeller' },
    { id: 'subtitles', label: 'Altyazı / Dublaj' },
    { id: 'upscale', label: 'Kalite Artırma' },
    { id: 'documents', label: 'Belge & Görsel' },
    { id: 'tasks', label: 'Görevler' },
    { id: 'diagnostics', label: 'Tanılama' },
    { id: 'settings', label: 'Ayarlar' },
  ];

  return (
    <div className="min-h-screen bg-[#242221] text-white flex flex-col">
      <header className="px-6 py-4 border-b border-white/10 flex items-center justify-between relative">
        <div className="flex items-center gap-3">
          <button
            onClick={() => setSidebarOpen(!sidebarOpen)}
            title="Sohbet geçmişini aç/kapa"
            className="shrink-0 hover:opacity-80 transition cursor-pointer"
          >
            <LogoOctagon className="w-10 h-10" />
          </button>
          <div>
            <h1 className="text-xl font-bold text-[#e95420]">Amorfly AI</h1>
            <p className="text-xs text-white/50">Yerel-öncelikli, bağımsız masaüstü asistanı</p>
          </div>
        </div>
        <nav className="flex gap-1 items-center">
          {tabs.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={
                'px-3 py-1.5 rounded text-sm transition ' +
                (tab === t.id ? 'bg-[#e95420] text-white' : 'text-white/60 hover:bg-white/10')
               + ' cursor-pointer'}
            >
              {t.label}
            </button>
          ))}
          <button
            onClick={() => setModeDrawerOpen(true)}
            title="Çalışma modu: Lokal / Hibrit / Online"
            className="ml-2 flex items-center gap-1 bg-white/10 hover:bg-white/20 rounded px-2 py-1.5 text-xs cursor-pointer"
          >
            {MODE_INFO.find((m) => m.id === workMode)?.icon}
            {MODE_INFO.find((m) => m.id === workMode)?.label}
            <ChevronLeft size={14} className={modeDrawerOpen ? 'rotate-180 transition' : 'transition'} />
          </button>
        </nav>

        {/* Sağdan açılan çalışma modu çekmecesi */}
        {modeDrawerOpen && (
          <>
            <div className="fixed inset-0 bg-black/40 z-40" onClick={() => setModeDrawerOpen(false)} />
            <div className="absolute top-full right-6 mt-2 w-80 bg-[#2d2b29] border border-white/10 rounded-lg shadow-xl z-50 p-3 space-y-2">
              <p className="text-xs text-white/40 px-1 mb-1">Çalışma modu — bu, Sohbet, Dil Eğitimi, Belge Analizi ve Excel Üretici için geçerli.</p>
              {MODE_INFO.map((m) => (
                <button
                  key={m.id}
                  onClick={() => { selectWorkMode(m.id); setModeDrawerOpen(false); }}
                  className={
                    'w-full text-left rounded-lg p-3 cursor-pointer transition ' +
                    (workMode === m.id ? 'bg-[#e95420]/20 border border-[#e95420]/50' : 'bg-white/5 hover:bg-white/10 border border-transparent')
                  }
                >
                  <div className="flex items-center gap-2 mb-1">
                    {m.icon}
                    <span className="text-sm font-medium">{m.label}</span>
                    {workMode === m.id && <span className="text-[10px] text-[#e95420] ml-auto">Aktif</span>}
                  </div>
                  <p className="text-xs text-white/50">{m.desc}</p>
                </button>
              ))}
            </div>
          </>
        )}
      </header>

      <main className="flex-1 p-6 overflow-auto">
        {tab === 'chat' && (
          <ChatTab
            sidebarOpen={sidebarOpen}
            setSidebarOpen={setSidebarOpen}
            workMode={workMode}
            onNavigateToExcel={(description) => {
              sessionStorage.setItem('pending_excel_description', description);
              setTab('documents');
            }}
          />
        )}
        {tab === 'language' && <LanguageLearningTab workMode={workMode} />}
        {tab === 'models' && <ModelsTab />}
        {tab === 'subtitles' && <SubtitlesTab />}
        {tab === 'upscale' && <UpscaleTab />}
        {tab === 'documents' && <DocumentsTab workMode={workMode} />}
        {tab === 'tasks' && <TasksTab />}
        {tab === 'diagnostics' && <DiagnosticsTab />}
        {tab === 'settings' && <SettingsTab />}
      </main>
    </div>
  );
}
