import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { PanelLeftClose, PanelLeftOpen, Plus, Trash2, ArrowDown, Mic, Volume2, FileDown, Loader2, Copy, Check, RefreshCw } from 'lucide-react';
import { logError } from '../lib/log';
import { whisperCodeFor } from '../lib/languages';
import { copyToClipboard } from '../lib/clipboard';
import ReactMarkdown from 'react-markdown';
import CodeBlock from '../components/CodeBlock';
import remarkGfm from 'remark-gfm';

type ChatMessage = { role: 'user' | 'assistant'; content: string };

interface Conversation {
  id: string;
  title: string;
  messages: ChatMessage[];
  updatedAt: number;
}

interface GpuInfo {
  vendor: string;
  model: string;
  total_vram_mb: number;
  free_vram_mb: number;
  source: string;
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

const CODE_EXT: Record<string, string> = {
  autolisp: 'lsp', lisp: 'lsp', lsp: 'lsp',
  python: 'py', py: 'py',
  bash: 'sh', sh: 'sh', shell: 'sh',
  sql: 'sql',
  vba: 'bas', basic: 'bas',
  javascript: 'js', js: 'js',
  typescript: 'ts', ts: 'ts',
  gcode: 'gcode',
  powershell: 'ps1', ps1: 'ps1',
  rust: 'rs', c: 'c', cpp: 'cpp', java: 'java',
};

const VAULT_KEY = 'conversations';
const VOICE_KEY = 'voice_settings';

/** Mesaj metnini ```dil ... ``` kod bloklarına ve düz metne ayırır. */
function parseSegments(text: string): { type: 'text' | 'code'; lang?: string; content: string }[] {
  const parts: { type: 'text' | 'code'; lang?: string; content: string }[] = [];
  const regex = /```(\w+)?\n([\s\S]*?)```/g;
  let last = 0;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(text))) {
    if (match.index > last) parts.push({ type: 'text', content: text.slice(last, match.index) });
    parts.push({ type: 'code', lang: match[1]?.toLowerCase() || 'txt', content: match[2] });
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push({ type: 'text', content: text.slice(last) });
  return parts;
}

/** Mesaj bir kod/programlama isteği gibi görünüyorsa, AI Router'daki
 * "kod" için ayrılmış modele geçmeyi öner — genel modeller kod
 * görevlerinde kod-odaklı modellerden (qwen2.5-coder gibi) genelde
 * daha tutarsız sonuç veriyor. */
function isCodeIntent(text: string): boolean {
  const t = text.toLocaleLowerCase('tr');
  return /\bkod\b|\bfonksiyon\b|\bscript\b|\bpython\b|\bjavascript\b|\btypescript\b|\brust\b|\bhata veriyor\b|\bdebug\b|\bprogramla|\bkodla/.test(t);
}

/** Sohbet sadece METİN üretebilir, dosya YAZAMAZ — kullanıcı bir Excel/
 * tablo istediğinde bunu fark edip gerçek Excel Üretici'ye (Belge &
 * Görsel sekmesi) yönlendirmek için basit anahtar kelime taraması.
 * Amaç: kullanıcının dakikalarca "sahte" bir metin cevabı beklemesini
 * önlemek — chat bir .xlsx dosyası asla üretemez. */
function isExcelIntent(text: string): boolean {
  const t = text.toLocaleLowerCase('tr');
  const hasExcelWord = /\bexcel\b|\bxlsx\b|\btablo\b|\btablosu\b|\bçizelge\b/.test(t);
  const hasCreateWord = /\byap\b|\boluştur|\bhazırla|\büret|\bver\b|\bistiyorum\b|\bistiyorum\b/.test(t);
  return hasExcelWord && hasCreateWord;
}

function CopyIconButton({ getText, small }: { getText: () => string; small?: boolean }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      onClick={async () => {
        const ok = await copyToClipboard(getText());
        if (ok) {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }
      }}
      title="Kopyala"
      className={
        'flex items-center gap-1 cursor-pointer hover:text-white ' +
        (small ? 'text-white/50' : 'text-white/30')
      }
    >
      {copied ? <Check size={13} /> : <Copy size={13} />}
      {small && (copied ? 'Kopyalandı' : 'Kopyala')}
    </button>
  );
}

interface ChatTabProps {
  sidebarOpen: boolean;
  setSidebarOpen: (v: boolean) => void;
  workMode: 'lokal' | 'hibrit' | 'online';
  onNavigateToExcel: (description: string) => void;
}

export default function ChatTab({ sidebarOpen, setSidebarOpen, workMode, onNavigateToExcel }: ChatTabProps) {
  const [ollamaOnline, setOllamaOnline] = useState<boolean | null>(null);
  const [installingOllama, setInstallingOllama] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [gpu, setGpu] = useState<GpuInfo | null>(null);
  const [voice, setVoice] = useState<VoiceSettings>(DEFAULT_VOICE);
  const [onlineConfigured, setOnlineConfigured] = useState(false);
  const [onlineLabel, setOnlineLabel] = useState('');
  // Not: eski yerel state'li "useOnline" geçişi kaldırıldı — mod artık
  // App.tsx'teki global Lokal/Hibrit/Online çekmecesinden (workMode) geliyor.

  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);

  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [refining, setRefining] = useState(false);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showScrollBtn, setShowScrollBtn] = useState(false);

  const scrollRef = useRef<HTMLDivElement>(null);
  const active = conversations.find((c) => c.id === activeId) || null;
  const messages = active?.messages ?? [];

  useEffect(() => {
    refreshStatus();
    loadConversations();
    loadVoiceSettings();
    invoke<{ label: string } | null>('get_online_provider').then((cfg) => {
      if (cfg && cfg.label) {
        setOnlineConfigured(true);
        setOnlineLabel(cfg.label);
      }
    }).catch(() => {});
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages.length, loading]);

  async function loadVoiceSettings() {
    try {
      const raw = await invoke<string | null>('vault_read', { name: VOICE_KEY });
      if (raw) setVoice({ ...DEFAULT_VOICE, ...JSON.parse(raw) });
    } catch {
      // henüz ayarlanmamış, varsayılanlarla devam
    }
  }

  async function loadConversations() {
    try {
      const raw = await invoke<string | null>('vault_read', { name: VAULT_KEY });
      if (raw) setConversations(JSON.parse(raw));
    } catch {
      // henüz kayıt yok, sorun değil — boş başlıyoruz
    }
  }

  async function persist(next: Conversation[]) {
    setConversations(next);
    try {
      await invoke('vault_write', { name: VAULT_KEY, plaintext: JSON.stringify(next) });
    } catch {
      // sessiz geç
    }
  }

  async function refreshStatus() {
    const online = await invoke<boolean>('check_ollama');
    setOllamaOnline(online);
    if (online) {
      const list = await invoke<string[]>('list_ollama_models');
      setModels(list);
      if (list.length > 0 && !selectedModel) {
        try {
          setSelectedModel(await invoke<string>('suggest_model_for_task', { task: 'genel', installedModels: list }));
        } catch {
          setSelectedModel(list[0]);
        }
      }
    }
    setGpu(await invoke<GpuInfo>('get_gpu_info'));
  }

  async function installOllama() {
    setInstallingOllama(true);
    setInstallError(null);
    try {
      await invoke('install_ollama_portable');
      await new Promise((r) => setTimeout(r, 2000));
      await refreshStatus();
    } catch (e) {
      setInstallError(await logError('ChatTab.installOllama', e));
    } finally {
      setInstallingOllama(false);
    }
  }

  function newChat() {
    setActiveId(null);
    setInput('');
    setError(null);
  }

  function deleteConversation(id: string) {
    const next = conversations.filter((c) => c.id !== id);
    persist(next);
    if (activeId === id) setActiveId(null);
  }

  function scrollToBottom(smooth = true) {
    requestAnimationFrame(() => {
      scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: smooth ? 'smooth' : 'auto' });
    });
  }

  function onScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    setShowScrollBtn(!atBottom);
  }

  async function recordVoiceInput() {
    if (recording) return;
    setRecording(true);
    setError(null);
    try {
      const text = await invoke<string>('record_and_transcribe', {
        durationSecs: voice.recordSeconds,
        whisperBin: voice.whisperBin,
        whisperModelPath: voice.whisperModelPath,
        language: whisperCodeFor(voice.responseLanguage),
      });
      setInput((prev) => (prev ? prev + ' ' + text : text));
    } catch (e) {
      setError(await logError('ChatTab.recordVoiceInput', e));
    } finally {
      setRecording(false);
    }
  }

  async function speakMessage(text: string) {
    try {
      await invoke('speak_text', { text, piperBin: voice.piperBin, voiceModel: voice.piperVoiceModel });
    } catch (e) {
      setError(await logError('ChatTab.speakMessage', e));
    }
  }

  async function saveCodeAsFile(code: string, lang: string) {
    const ext = CODE_EXT[lang] || 'txt';
    const path = await saveDialog({ defaultPath: `amorfly_kod.${ext}` });
    if (path) {
      await writeTextFile(path, code);
    }
  }

  async function sendMessage() {
    if (!input.trim() || loading) return;
    const text = input;
    setInput('');
    await runSend(text);
  }

  /** Belirli bir kullanıcı mesajını (retryIndex) yeniden gönderir: o
   * mesajdan SONRAKİ her şeyi (eski/yanlış cevap dahil) siler, aynı
   * metni tekrar gönderip taze bir cevap alır. */
  async function retryFromMessage(retryIndex: number) {
    if (!active || loading) return;
    const msg = active.messages[retryIndex];
    if (!msg || msg.role !== 'user') return;
    const truncated = active.messages.slice(0, retryIndex);
    const truncatedConvo: Conversation = { ...active, messages: truncated, updatedAt: Date.now() };
    persist([truncatedConvo, ...conversations.filter((c) => c.id !== truncatedConvo.id)]);
    await runSend(msg.content, truncatedConvo);
  }

  async function runSend(text: string, baseConvo?: Conversation) {
    let convo = baseConvo ?? active;
    const userMsg: ChatMessage = { role: 'user', content: text };

    if (!convo) {
      convo = {
        id: crypto.randomUUID(),
        title: text.trim().slice(0, 42) + (text.trim().length > 42 ? '…' : ''),
        messages: [],
        updatedAt: Date.now(),
      };
      setActiveId(convo.id);
    }

    const withUser: Conversation = { ...convo, messages: [...convo.messages, userMsg], updatedAt: Date.now() };
    const listWithUser = [withUser, ...conversations.filter((c) => c.id !== withUser.id)];
    persist(listWithUser);

    setLoading(true);
    setError(null);
    scrollToBottom();

    // Dil/akıcılık talimatı + ortak hafıza özeti (geçmiş işlemler/tercihler):
    // devrik cümleleri önler ve modele geçmişten kısa, AI tarafından
    // özetlenmiş anlamlı bir bağlam verir (ham kayıt listesi değil).
    let memoryNote = '';
    try {
      memoryNote = await invoke<string>('summarize_memory', { model: selectedModel });
    } catch {
      // hafıza henüz yok/okunamadı — sorun değil, boş devam
    }

    const languageInstruction = {
      role: 'system',
      content:
        `Her zaman ${voice.responseLanguage} dilinde, dilbilgisi kurallarına tam uygun, akıcı ve doğal cümlelerle cevap ver. Devrik, yarım ya da anlamsız cümle kurma.` +
        (memoryNote ? `\n\n${memoryNote}` : ''),
    };

    try {
      const fullMessages: { role: string; content: string }[] = [languageInstruction, ...withUser.messages];

      const reply = await invoke<string>('smart_chat', {
        mode: workMode,
        model: selectedModel,
        messages: fullMessages,
      });

      let finalText = reply;
      if (voice.autoRefine && ollamaOnline) {
        setRefining(true);
        try {
          finalText = await invoke<string>('refine_language', {
            text: reply,
            model: selectedModel,
            targetLanguage: voice.responseLanguage,
          });
        } catch {
          finalText = reply; // düzeltme başarısız olursa orijinal cevap kalır
        } finally {
          setRefining(false);
        }
      }

      const withReply: Conversation = {
        ...withUser,
        messages: [...withUser.messages, { role: 'assistant', content: finalText }],
        updatedAt: Date.now(),
      };
      persist([withReply, ...listWithUser.filter((c) => c.id !== withReply.id)]);

      if (voice.autoSpeak) speakMessage(finalText);
    } catch (e) {
      setError(await logError('ChatTab.sendMessage', e));
    } finally {
      setLoading(false);
    }
  }

  const sorted = [...conversations].sort((a, b) => b.updatedAt - a.updatedAt);

  return (
    <div className="flex gap-4 h-[calc(100vh-140px)]">
      <aside className={'transition-all overflow-hidden flex flex-col ' + (sidebarOpen ? 'w-56' : 'w-0')}>
        <div className="w-56 flex flex-col h-full">
          <button onClick={newChat} className="flex items-center gap-2 bg-[#e95420] rounded-lg px-3 py-2 text-sm mb-3 shrink-0 cursor-pointer">
            <Plus size={16} /> Yeni Sohbet
          </button>
          <p className="text-xs text-white/30 uppercase tracking-wide px-2 mb-1 shrink-0">Geçmiş</p>
          <div className="flex-1 overflow-y-auto space-y-1">
            {sorted.length === 0 && <p className="text-white/30 text-xs px-2">Henüz sohbet yok.</p>}
            {sorted.map((c) => (
              <div
                key={c.id}
                onClick={() => setActiveId(c.id)}
                className={
                  'group flex items-center justify-between gap-1 px-2 py-2 rounded-lg cursor-pointer text-sm ' +
                  (c.id === activeId ? 'bg-white/15' : 'hover:bg-white/5')
                }
              >
                <span className="truncate">{c.title}</span>
                <button onClick={(e) => { e.stopPropagation(); deleteConversation(c.id); }} className="opacity-0 group-hover:opacity-60 hover:!opacity-100 shrink-0 cursor-pointer" title="Sil">
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
          </div>
        </div>
      </aside>

      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex items-center justify-between mb-3">
          <button onClick={() => setSidebarOpen(!sidebarOpen)} className="text-white/50 hover:text-white" title="Sohbet listesi">
            {sidebarOpen ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}
          </button>
          <div className="flex items-center gap-2">
            <span className="text-xs text-white/30">{voice.responseLanguage}</span>
            <span className="text-xs rounded px-2 py-1 bg-white/10 text-white/50" title="Çalışma modunu sağ üstteki menüden değiştirebilirsin">
              {workMode === 'lokal' ? '● Lokal' : workMode === 'hibrit' ? '✦ Hibrit' : '☁ Online'}
            </span>
            {models.length > 0 && workMode !== 'online' && (
              <select value={selectedModel} onChange={(e) => setSelectedModel(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1 text-sm">
                {models.map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            )}
          </div>
        </div>

        <section className="mb-3 grid grid-cols-1 sm:grid-cols-2 gap-3 shrink-0">
          <div className="bg-white/5 rounded-lg p-3 text-sm">
            {ollamaOnline === null && <p className="text-white/50">Kontrol ediliyor…</p>}
            {ollamaOnline === true && <p className="text-green-400">● Ollama bağlı — {models.length} model</p>}
            {ollamaOnline === false && (
              <div>
                <p className="text-red-400 mb-2">● Ollama bağlı değil</p>
                <button onClick={installOllama} disabled={installingOllama} className="bg-[#e95420] rounded px-3 py-1.5 text-xs disabled:opacity-40 cursor-pointer">
                  {installingOllama ? 'Kuruluyor…' : "Ollama'yı Otomatik Kur"}
                </button>
                {installError && <p className="text-red-400 text-xs mt-1">{installError}</p>}
                <div className="mt-2 bg-black/30 rounded p-2">
                  <p className="text-white/40 text-xs mb-1">Otomatik kurulum çalışmazsa, terminale yapıştır:</p>
                  <div className="flex items-center gap-2">
                    <code className="flex-1 bg-black/40 rounded px-2 py-1 text-xs text-white/70 overflow-x-auto whitespace-nowrap">
                      curl -fsSL https://ollama.com/install.sh | sh
                    </code>
                    <button
                      onClick={() => copyToClipboard('curl -fsSL https://ollama.com/install.sh | sh')}
                      className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-2 py-1 text-xs cursor-pointer"
                    >
                      Kopyala
                    </button>
                  </div>
                  <p className="text-white/30 text-xs mt-1">
                    Çalıştırdıktan sonra bu pencereyi kapatıp aç — otomatik algılanır.
                  </p>
                </div>
              </div>
            )}
          </div>
          <div className="bg-white/5 rounded-lg p-3 text-sm text-white/60">
            {gpu ? `${gpu.vendor} — ${gpu.model}` : 'Donanım tespit ediliyor…'}
          </div>
        </section>

        <div className="relative flex-1 min-h-0">
          <div ref={scrollRef} onScroll={onScroll} className="bg-white/5 rounded-lg p-4 h-full overflow-y-auto flex flex-col gap-3">
            {messages.length === 0 && !loading && (
              <div className="flex-1 flex items-center justify-center text-center">
                <div>
                  <p className="text-white/40 text-lg mb-1">Ne hakkında konuşmak istersin?</p>
                  <p className="text-white/25 text-xs">Yerel modelin, hiçbir şey dışarı gönderilmeden yanıt verir.</p>
                </div>
              </div>
            )}
            {messages.map((m, i) => (
              <div key={i} className={m.role === 'user' ? 'text-right' : 'text-left'}>
                {m.role === 'assistant' ? (
                  <div className="inline-block max-w-[85%] text-left">
                    {parseSegments(m.content).map((seg, j) =>
                      seg.type === 'code' ? (
                        <CodeBlock
                          key={j}
                          code={seg.content}
                          language={seg.lang}
                          extraAction={
                            <button onClick={() => saveCodeAsFile(seg.content, seg.lang || 'txt')} className="flex items-center gap-1 hover:text-white cursor-pointer">
                              <FileDown size={13} /> Dosya Olarak Kaydet
                            </button>
                          }
                        />
                      ) : (
                        seg.content.trim() && (
                          <div
                            key={j}
                            className="inline-block rounded-lg px-3 py-2 bg-white/10 text-sm [&_table]:border-collapse [&_td]:border [&_td]:border-white/20 [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-white/20 [&_th]:px-2 [&_th]:py-1 [&_th]:bg-white/10 [&_p]:my-1 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:pl-5 [&_code]:bg-black/30 [&_code]:rounded [&_code]:px-1"
                          >
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{seg.content}</ReactMarkdown>
                          </div>
                        )
                      )
                    )}
                    <div className="flex items-center justify-between mt-1">
                      <button onClick={() => speakMessage(m.content)} className="text-white/30 hover:text-white cursor-pointer" title="Sesli oku">
                        <Volume2 size={14} />
                      </button>
                      <CopyIconButton getText={() => m.content} />
                    </div>
                  </div>
                ) : (
                  <div>
                    <span className="inline-block rounded-lg px-3 py-2 max-w-[80%] text-sm bg-[#e95420]/80">{m.content}</span>
                    <div>
                      <button
                        onClick={() => retryFromMessage(i)}
                        disabled={loading}
                        title="Bu isteği tekrar gönder (cevap yanlış/eksikse)"
                        className="text-white/30 hover:text-white cursor-pointer disabled:opacity-30 mt-1"
                      >
                        <RefreshCw size={13} />
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
            {loading && <p className="text-white/40 text-sm">Yerel model düşünüyor…</p>}
            {refining && <p className="text-white/30 text-xs">Dil/akıcılık düzeltiliyor…</p>}
            {error && <p className="text-red-400 text-sm bg-red-950/30 rounded p-2">{error}</p>}
          </div>

          {showScrollBtn && (
            <button onClick={() => scrollToBottom()} className="absolute bottom-3 right-3 bg-[#e95420] rounded-full p-2 shadow-lg hover:brightness-110 cursor-pointer" title="En alta git">
              <ArrowDown size={16} />
            </button>
          )}
        </div>

        {isCodeIntent(input) && !isExcelIntent(input) && models.length > 0 && (
          <div className="mb-2 bg-white/5 border border-white/10 rounded-lg p-2.5 text-xs flex items-center justify-between gap-2 shrink-0">
            <span className="text-white/60">Bu bir kod isteği gibi görünüyor — kod-odaklı bir model daha tutarlı sonuç verebilir.</span>
            <button
              onClick={async () => {
                try {
                  setSelectedModel(await invoke<string>('suggest_model_for_task', { task: 'kod', installedModels: models }));
                } catch { /* sessizce yoksay, mevcut model kalsın */ }
              }}
              className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-3 py-1.5 cursor-pointer whitespace-nowrap"
            >
              Kod Modeline Geç
            </button>
          </div>
        )}
        {isExcelIntent(input) && (
          <div className="mb-2 bg-[#e95420]/10 border border-[#e95420]/30 rounded-lg p-2.5 text-xs flex items-center justify-between gap-2 shrink-0">
            <span className="text-white/70">
              Bu bir Excel/tablo isteği gibi görünüyor — sohbet dosya <strong>üretemez</strong>, sadece
              metinle cevap verir. Gerçek, formüllü bir .xlsx için Excel Üretici'yi kullan.
            </span>
            <button
              onClick={() => onNavigateToExcel(input)}
              className="shrink-0 bg-[#e95420] rounded px-3 py-1.5 cursor-pointer whitespace-nowrap"
            >
              Excel Üretici'ye Git
            </button>
          </div>
        )}
        <div className="flex gap-2 mt-3 shrink-0">
          <button
            onClick={recordVoiceInput}
            disabled={recording}
            title={`${voice.recordSeconds} saniye kaydet`}
            className={'rounded px-3 py-2 ' + (recording ? 'bg-red-500 animate-pulse' : 'bg-white/10 hover:bg-white/20') + ' cursor-pointer'}
          >
            {recording ? <Loader2 size={16} className="animate-spin" /> : <Mic size={16} />}
          </button>
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && sendMessage()}
            placeholder="Bir şey sorun ya da mikrofonla konuşun…"
            className="flex-1 bg-white/10 text-white rounded px-3 py-2 outline-none text-sm"
          />
          <button onClick={sendMessage} disabled={loading || (workMode !== 'online' && !ollamaOnline)} className="bg-[#e95420] rounded px-4 py-2 text-sm disabled:opacity-40 cursor-pointer">
            Gönder
          </button>
        </div>
      </div>
    </div>
  );
}
