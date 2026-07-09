import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Mic, Volume2, Loader2, Plus, Trash2, ImagePlus, GraduationCap } from 'lucide-react';
import { LANGUAGES, whisperCodeFor } from '../lib/languages';
import { logError } from '../lib/log';
import MessageContent from '../components/MessageContent';

interface LangMessage {
  role: 'user' | 'assistant';
  content: string;
}

interface LangSession {
  id: string;
  dil: string;
  seviye: string;
  senaryo: string;
  mesajlar: LangMessage[];
  son_guncelleme: string;
}

interface VoiceSettings {
  whisperBin: string;
  whisperModelPath: string;
  piperBin: string;
  piperVoiceModel: string;
  recordSeconds: number;
}

const LEVELS = ['A1 (Başlangıç)', 'A2 (Temel)', 'B1 (Orta)', 'B2 (Orta-Üst)', 'C1 (İleri)'];
const VOICE_KEY = 'voice_settings';
const CORRECTION_MARKER = '###DUZELTME###';

const LEVEL_RESULT_MARKER = '###SEVIYE_SONUC###';
// Bir oturum henüz seviye testinden geçmediyse "seviye" alanı bu değeri taşır.
const PENDING_LEVEL = 'Seviye Belirleniyor';

/** Latin alfabesi kullanmayan diller — bu diller için modelin HER ZAMAN
 * kendi yazı sistemiyle (Kiril, Arap alfabesi vb.) yazması gerekiyor,
 * öğrenci Latin harfleriyle (transliterasyon, ör. Rusça için "privet")
 * yazsa bile. Önceki sürümde bu ayrım yoktu — model bazen Latin harfli
 * "okunuş" yazıyor ya da kısa selamlaşmalarda konudan sapıp saçmalıyordu. */
const NON_LATIN_SCRIPT_HINT: Record<string, string> = {
  'Rusça': 'Kiril alfabesi (örn. "Привет", "Как дела?")',
  'Ukraynaca': 'Kiril alfabesi',
  'Belarusça': 'Kiril alfabesi',
  'Bulgarca': 'Kiril alfabesi',
  'Sırpça': 'Kiril alfabesi',
  'Makedonca': 'Kiril alfabesi',
  'Arapça': 'Arap alfabesi',
  'Farsça': 'Arap alfabesi (Fars yazısı)',
  'Peştuca': 'Arap alfabesi',
  'Sindçe': 'Arap alfabesi',
  'İbranice': 'İbrani alfabesi',
  'Çince': 'Çince karakterler (Hanzi)',
  'Kantonca': 'Çince karakterler (Hanzi)',
  'Japonca': 'Japon yazı sistemi (Hiragana/Katakana/Kanji karışık)',
  'Korece': 'Hangıl alfabesi',
  'Yunanca': 'Yunan alfabesi',
  'Ermenice': 'Ermeni alfabesi',
  'Gürcüce': 'Gürcü alfabesi',
  'Tayca': 'Tay alfabesi',
  'Kmerce': 'Khmer alfabesi',
  'Laoca': 'Lao alfabesi',
  'Birmanca': 'Birma alfabesi',
  'Hintçe': 'Devanagari alfabesi',
  'Bengalce': 'Bengal alfabesi',
  'Tamilce': 'Tamil alfabesi',
};

function scriptInstruction(dil: string): string {
  const script = NON_LATIN_SCRIPT_HINT[dil];
  if (!script) return '';
  return (
    `\n\nÇOK ÖNEMLİ — YAZI SİSTEMİ: ${dil} dili Latin alfabesi kullanmaz, ${script} kullanır. ` +
    `SEN HER ZAMAN bu alfabeyle yaz, asla Latin harfleriyle "okunuşunu" yazma. Öğrenci Latin ` +
    `harfleriyle yazsa bile (örn. klavyesi yoksa "privet" gibi transliterasyon yazabilir), bunu ` +
    `o dili öğrenmeye çalıştığının bir işareti olarak anla ve doğru alfabeyle nazikçe düzelt/devam et.`
  );
}

/** Seviye tespit sınavı modu: normal ders sohbeti değil, adaptif bir
 * mini-sınav. Model artan/azalan zorlukta sorular sorar, öğrencinin
 * cevaplarına göre zorluğu ayarlar, 5 soru sonunda CEFR seviyesini
 * (A1-C1) ve kısa bir ders programını LEVEL_RESULT_MARKER formatında
 * kesin olarak bildirir — bu, frontend'in oturumu otomatik "gerçek"
 * bir derse dönüştürmesi için ayrıştırılıyor. */
function placementTestSystemPrompt(dil: string): string {
  return (
    `Sen ${dil} dili için seviye tespit sınavı yapan bir öğretmensin.${scriptInstruction(dil)}\n` +
    `Öğrenciye SIRAYLA, artan zorlukta TOPLAM 5 kısa soru sor — ilk soru çok basit (A1), ` +
    `öğrenci doğru cevaplarsa bir sonraki soruyu zorlaştır, yanlış cevaplarsa basitleştir ` +
    `(adaptif test). HER SEFERİNDE SADECE BİR SORU SOR ve öğrencinin cevabını bekle — ` +
    `art arda birden fazla soru sorma, açıklama yapma, sadece soruyu yaz.\n` +
    `5. sorunun cevabını aldıktan sonra, ÖNCE öğrenciye 1-2 cümle Türkçe kısa bir değerlendirme yaz, ` +
    `SONRA cevabının EN SONUNA, başka HİÇBİR ŞEY eklemeden tam olarak şu formatta kesin sonucu ekle:\n` +
    `${LEVEL_RESULT_MARKER}\n` +
    `{"seviye": "A1 (Başlangıç)" ya da "A2 (Temel)" ya da "B1 (Orta)" ya da "B2 (Orta-Üst)" ya da "C1 (İleri)", ` +
    `"ders_plani": ["ilk konu", "ikinci konu", "üçüncü konu"]}\n` +
    `Bu JSON kesinlikle geçerli, tek satırlık ve tam bu alanlarla olmalı.`
  );
}

interface LanguageLearningTabProps {
  workMode: 'lokal' | 'hibrit' | 'online';
}

export default function LanguageLearningTab({ workMode }: LanguageLearningTabProps) {
  const [sessions, setSessions] = useState<LangSession[]>([]);
  const [active, setActive] = useState<LangSession | null>(null);
  const [loadingSessions, setLoadingSessions] = useState(true);

  // Yeni oturum formu
  const [newDil, setNewDil] = useState('İngilizce');
  const [newSeviye, setNewSeviye] = useState(LEVELS[2]);
  const [newSenaryo, setNewSenaryo] = useState('');

  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [voice, setVoice] = useState<VoiceSettings>({
    whisperBin: 'whisper-cli',
    whisperModelPath: '',
    piperBin: 'piper',
    piperVoiceModel: 'tr_TR-dfki-medium',
    recordSeconds: 6,
  });
  const [model, setModel] = useState('llama3.2');
  const [models, setModels] = useState<string[]>([]);

  const [imagePath, setImagePath] = useState<string | null>(null);
  const [imageUrl, setImageUrl] = useState<string | null>(null);

  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadSessions();
    loadVoiceSettings();
    invoke<string[]>('list_ollama_models').then((list) => {
      setModels(list);
      if (list.length > 0) {
        invoke<string>('suggest_model_for_task', { task: 'dil_egitimi', installedModels: list })
          .then(setModel)
          .catch(() => setModel(list[0]));
      }
    }).catch(() => {});
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [active?.mesajlar.length]);

  async function loadVoiceSettings() {
    try {
      const raw = await invoke<string | null>('vault_read', { name: VOICE_KEY });
      if (raw) setVoice((prev) => ({ ...prev, ...JSON.parse(raw) }));
    } catch {
      // ayarlar henüz yok — varsayılanla devam
    }
  }

  async function loadSessions() {
    setLoadingSessions(true);
    try {
      const list = await invoke<LangSession[]>('list_language_sessions');
      setSessions(list);
    } catch (e) {
      setError(await logError('LanguageLearningTab.loadSessions', e));
    } finally {
      setLoadingSessions(false);
    }
  }

  async function startNewSession() {
    try {
      const s = await invoke<LangSession>('create_language_session', {
        dil: newDil,
        seviye: newSeviye,
        senaryo: newSenaryo.trim() || 'Serbest sohbet',
      });
      setSessions((prev) => [s, ...prev]);
      setActive(s);
    } catch (e) {
      setError(await logError('LanguageLearningTab.startNewSession', e));
    }
  }

  async function deleteSession(id: string) {
    try {
      await invoke('delete_language_session', { id });
      setSessions((prev) => prev.filter((s) => s.id !== id));
      if (active?.id === id) setActive(null);
    } catch (e) {
      setError(await logError('LanguageLearningTab.deleteSession', e));
    }
  }

  function systemPrompt(session: LangSession): string {
    return (
      `Sen bir dil öğretmenisin. Öğrenciyle SADECE ${session.dil} dilinde, ${session.seviye} seviyesine uygun ` +
      `kelime/dilbilgisi karmaşıklığında konuşuyorsun. Senaryo/konu: "${session.senaryo}".${scriptInstruction(session.dil)}\n` +
      `Konuşmayı doğal şekilde sürdür, açık uçlu sorular sor. Öğrencinin söylediğiyle İLGİSİZ, konudan sapan, ` +
      `uydurma bir cevap ASLA verme — anlamadıysan ya da metin belirsizse, öğrenciye o dilde kibarca ne demek ` +
      `istediğini sor, tahmin yürütüp saçmalama. Öğrencinin yazdığı/söylediği metinde dilbilgisi ya da kelime ` +
      `hatası varsa, cevabının EN SONUNA şu formatta bir düzeltme ekle (hata yoksa bu kısmı HİÇ ekleme):\n` +
      `${CORRECTION_MARKER}\n[Yanlış]: (öğrencinin cümlesi) -> [Doğrusu]: (doğru hali) — (çok kısa, 1 cümlelik açıklama, Türkçe)`
    );
  }

  async function sendMessage(text: string) {
    if (!active || !text.trim() || busy) return;
    setBusy(true);
    setError(null);
    const userMsg: LangMessage = { role: 'user', content: text.trim() };
    const updated = { ...active, mesajlar: [...active.mesajlar, userMsg] };
    setActive(updated);
    setInput('');

    const inTestMode = active.seviye === PENDING_LEVEL;

    try {
      const ollamaMessages = [
        { role: 'system', content: inTestMode ? placementTestSystemPrompt(active.dil) : systemPrompt(active) },
        ...updated.mesajlar.map((m) => ({ role: m.role, content: m.content })),
      ];
      const reply = await invoke<string>('smart_chat', { mode: workMode, model, messages: ollamaMessages });

      let displayReply = reply;
      let newSeviye = active.seviye;
      let newSenaryo = active.senaryo;

      if (inTestMode && reply.includes(LEVEL_RESULT_MARKER)) {
        const [before, after] = reply.split(LEVEL_RESULT_MARKER);
        displayReply = before.trim();
        try {
          const parsed = JSON.parse(after.trim());
          if (parsed.seviye) newSeviye = parsed.seviye;
          if (Array.isArray(parsed.ders_plani) && parsed.ders_plani.length > 0) {
            newSenaryo = `Ders programı: ${parsed.ders_plani.join(' → ')}`;
          }
        } catch {
          // JSON ayrıştırılamadıysa seviye "Belirleniyor" kalır, kullanıcı elle seçebilir
        }
      }

      const finalMsgs = [...updated.mesajlar, { role: 'assistant' as const, content: displayReply }];
      const finalSession = { ...updated, mesajlar: finalMsgs, seviye: newSeviye, senaryo: newSenaryo };
      setActive(finalSession);
      await invoke('save_language_session', { id: active.id, mesajlar: finalMsgs });
      if (newSeviye !== active.seviye || newSenaryo !== active.senaryo) {
        await invoke('update_language_session_meta', { id: active.id, seviye: newSeviye, senaryo: newSenaryo }).catch(() => {});
      }
      setSessions((prev) => prev.map((s) => (s.id === active.id ? finalSession : s)));
    } catch (e) {
      setError(await logError('LanguageLearningTab.sendMessage', e));
    } finally {
      setBusy(false);
    }
  }

  /** Seviyeni bilmiyorsan: elle seçmek yerine kısa, adaptif bir sınavla
   * başlar. Sınav bitince (LEVEL_RESULT_MARKER algılanınca) oturum
   * otomatik olarak gerçek bir derse dönüşür — seviye kilitlenir, kısa
   * bir ders programı önerisiyle. */
  async function startPlacementTest() {
    try {
      const s = await invoke<LangSession>('create_language_session', {
        dil: newDil,
        seviye: PENDING_LEVEL,
        senaryo: 'Seviye Tespit Sınavı',
      });
      setSessions((prev) => [s, ...prev]);
      setActive(s);
      setBusy(true);
      const kickoff = [
        { role: 'system', content: placementTestSystemPrompt(s.dil) },
        { role: 'user', content: '(Sınavı başlat, ilk soruyu sor.)' },
      ];
      const reply = await invoke<string>('smart_chat', { mode: workMode, model, messages: kickoff });
      // Savunmacı: model beklenmedik şekilde ilk yanıtta sonucu verirse
      // (olmaması gerekir ama), ham JSON'u kullanıcıya göstermeyelim.
      const displayReply = reply.includes(LEVEL_RESULT_MARKER) ? reply.split(LEVEL_RESULT_MARKER)[0].trim() : reply;
      const finalMsgs: LangMessage[] = [{ role: 'assistant', content: displayReply }];
      const finalSession = { ...s, mesajlar: finalMsgs };
      setActive(finalSession);
      await invoke('save_language_session', { id: s.id, mesajlar: finalMsgs });
      setSessions((prev) => prev.map((x) => (x.id === s.id ? finalSession : x)));
    } catch (e) {
      setError(await logError('LanguageLearningTab.startPlacementTest', e));
    } finally {
      setBusy(false);
    }
  }

  async function recordVoice() {
    if (recording || !active) return;
    setRecording(true);
    setError(null);
    try {
      const text = await invoke<string>('record_and_transcribe', {
        durationSecs: voice.recordSeconds,
        whisperBin: voice.whisperBin,
        whisperModelPath: voice.whisperModelPath,
        language: whisperCodeFor(active.dil),
      });
      if (text.trim()) await sendMessage(text);
    } catch (e) {
      setError(await logError('LanguageLearningTab.recordVoice', e));
    } finally {
      setRecording(false);
    }
  }

  async function speak(text: string) {
    try {
      await invoke('speak_text', { text, piperBin: voice.piperBin, voiceModel: voice.piperVoiceModel });
    } catch (e) {
      setError(await logError('LanguageLearningTab.speak', e));
    }
  }

  async function pickImage() {
    const file = await open({ multiple: false, filters: [{ name: 'Görsel', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
    if (typeof file === 'string') {
      setImagePath(file);
      setImageUrl(convertFileSrc(file));
    }
  }

  async function askAboutImage() {
    if (!imagePath || !active) return;
    setBusy(true);
    setError(null);
    try {
      const question = `${active.dil} dilinde, ${active.seviye} seviyesine uygun şekilde bu görseldeki şeyi sor (ör: "What is this?" tarzı, hedef dilde). Sadece soruyu yaz, başka bir şey yazma.`;
      const askText = await invoke<string>('analyze_image', { path: imagePath, question, model: 'llava:7b' });
      const userVisibleMsg: LangMessage = { role: 'assistant', content: askText };
      const updated = { ...active, mesajlar: [...active.mesajlar, userVisibleMsg] };
      setActive(updated);
      await invoke('save_language_session', { id: active.id, mesajlar: updated.mesajlar });
      setSessions((prev) => prev.map((s) => (s.id === active.id ? updated : s)));
    } catch (e) {
      setError(await logError('LanguageLearningTab.askAboutImage', e));
    } finally {
      setBusy(false);
      setImagePath(null);
      setImageUrl(null);
    }
  }

  // ---------- Oturum listesi / yeni oturum ekranı ----------
  if (!active) {
    return (
      <div className="max-w-2xl space-y-5">
        <div className="flex items-center gap-2">
          <GraduationCap size={18} className="text-[#e95420]" />
          <h2 className="font-semibold">Dil Eğitimi</h2>
        </div>
        <p className="text-white/50 text-sm">
          Yazışarak ya da mikrofonla konuşarak pratik yap. İlerlemen otomatik kaydedilir, ertesi gün
          kaldığın yerden devam edersin. <strong>Not:</strong> telaffuz analizi yapmaz (whisper sesi metne
          çevirir, model sadece metni görür) — dilbilgisi/kelime/akıcılık pratiği içindir.
        </p>

        <div className="bg-white/5 rounded-lg p-4 space-y-3">
          <p className="text-sm font-medium">Yeni Ders Başlat</p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-white/50 text-xs">Dil</span>
              <select value={newDil} onChange={(e) => setNewDil(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1.5 cursor-pointer">
                {LANGUAGES.map((l) => <option key={l.whisper} value={l.label}>{l.label}</option>)}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-white/50 text-xs">Seviye</span>
              <select value={newSeviye} onChange={(e) => setNewSeviye(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1.5 cursor-pointer">
                {LEVELS.map((l) => <option key={l} value={l}>{l}</option>)}
              </select>
            </label>
          </div>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-white/50 text-xs">Senaryo / konu (opsiyonel — seviye testinde kullanılmaz)</span>
            <input
              value={newSenaryo}
              onChange={(e) => setNewSenaryo(e.target.value)}
              placeholder="Ör: restoranda sipariş verme, iş görüşmesi, serbest sohbet…"
              className="bg-white/10 text-white rounded px-2 py-1.5"
            />
          </label>
          <div className="flex flex-wrap gap-2">
            <button onClick={startNewSession} className="flex items-center gap-1 bg-white/10 rounded px-4 py-2 text-sm cursor-pointer">
              <Plus size={14} /> Seçtiğim Seviyeyle Başla
            </button>
            <button onClick={startPlacementTest} className="flex items-center gap-1 bg-[#e95420] rounded px-4 py-2 text-sm cursor-pointer">
              <GraduationCap size={14} /> Önce Seviyemi Test Et (Önerilen)
            </button>
          </div>
          <p className="text-xs text-white/30">
            Seviye testi: {newDil} dilinde 5 kısa, artan zorlukta soru sorar; cevaplarına göre CEFR
            seviyeni (A1-C1) belirler ve sana uygun bir ders programıyla derse başlar.
          </p>
        </div>

        {loadingSessions ? (
          <p className="text-white/30 text-sm">Yükleniyor…</p>
        ) : sessions.length > 0 ? (
          <div>
            <p className="text-sm font-medium mb-2">Kaldığın Yerden Devam Et</p>
            <div className="space-y-2">
              {sessions.map((s) => (
                <div key={s.id} className="bg-white/5 rounded-lg p-3 flex items-center justify-between">
                  <button onClick={() => setActive(s)} className="text-left flex-1 cursor-pointer">
                    <p className="text-sm">
                      {s.dil} · {s.seviye === PENDING_LEVEL ? <span className="text-[#e95420]">Seviye testi yarım kaldı</span> : s.seviye}
                    </p>
                    <p className="text-xs text-white/40">{s.senaryo} — {s.mesajlar.length} mesaj</p>
                  </button>
                  <button onClick={() => deleteSession(s.id)} className="text-white/30 hover:text-red-400 cursor-pointer p-2">
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        ) : null}

        {error && <p className="text-red-400 text-sm bg-red-950/30 rounded p-3">{error}</p>}
      </div>
    );
  }

  // ---------- Aktif ders ekranı ----------
  return (
    <div className="max-w-3xl flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <div>
          <p className="text-sm font-medium">
            {active.dil} · {active.seviye === PENDING_LEVEL ? (
              <span className="text-[#e95420]">Seviye Tespit Sınavı Sürüyor…</span>
            ) : active.seviye}
          </p>
          <p className="text-xs text-white/40">{active.senaryo}</p>
        </div>
        <div className="flex items-center gap-2">
          <select value={model} onChange={(e) => setModel(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1 text-xs cursor-pointer">
            {models.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
          <button onClick={() => setActive(null)} className="text-xs bg-white/10 rounded px-3 py-1.5 cursor-pointer">
            Oturum Listesi
          </button>
        </div>
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto space-y-3 pb-3">
        {active.mesajlar.length === 0 && (
          <p className="text-white/30 text-sm text-center py-8">
            Mikrofon ya da yazarak dersi başlat.
          </p>
        )}
        {active.mesajlar.map((m, i) => {
          if (m.role === 'user') {
            return (
              <div key={i} className="text-right">
                <span className="inline-block max-w-[85%] rounded-lg px-3 py-2 bg-[#e95420]/80 text-sm text-left">
                  {m.content}
                </span>
              </div>
            );
          }
          const [reply, correction] = m.content.split(CORRECTION_MARKER);
          return (
            <div key={i} className="text-left">
              <div className="inline-block max-w-[85%] bg-white/10 rounded-lg px-3 py-2">
                <MessageContent text={reply.trim()} />
              </div>
              {correction && correction.trim() && (
                <div className="max-w-[85%] mt-1 bg-[#e95420]/10 border border-[#e95420]/30 rounded-lg px-3 py-2 text-xs text-white/70">
                  <span className="text-[#e95420] font-medium">Düzeltme: </span>
                  {correction.trim()}
                </div>
              )}
              <button onClick={() => speak(reply.trim())} className="text-white/30 hover:text-white cursor-pointer mt-1" title="Sesli oku">
                <Volume2 size={14} />
              </button>
            </div>
          );
        })}
        {busy && <Loader2 size={16} className="animate-spin text-white/40" />}
      </div>

      {imageUrl && (
        <div className="mb-2 bg-white/5 rounded-lg p-2 flex items-center gap-2">
          <img src={imageUrl} alt="" className="w-14 h-14 object-cover rounded" />
          <button onClick={askAboutImage} disabled={busy} className="bg-[#e95420] rounded px-3 py-1.5 text-xs cursor-pointer disabled:opacity-40">
            Bu Görsel Hakkında Soru Sor
          </button>
          <button onClick={() => { setImagePath(null); setImageUrl(null); }} className="text-white/40 text-xs cursor-pointer">
            Vazgeç
          </button>
        </div>
      )}

      {error && <p className="text-red-400 text-xs mb-2">{error}</p>}

      <div className="flex items-center gap-2">
        <button onClick={pickImage} title="Görsel ile kelime pratiği" className="bg-white/10 rounded p-2 cursor-pointer">
          <ImagePlus size={16} />
        </button>
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && sendMessage(input)}
          placeholder={`${active.dil} dilinde yaz…`}
          className="flex-1 bg-white/10 text-white rounded px-3 py-2 text-sm outline-none"
        />
        <button
          onClick={recordVoice}
          disabled={recording}
          className={'rounded p-2 cursor-pointer ' + (recording ? 'bg-red-500 animate-pulse' : 'bg-white/10 hover:bg-white/20')}
          title="Mikrofonla konuş"
        >
          <Mic size={16} />
        </button>
        <button onClick={() => sendMessage(input)} disabled={busy || !input.trim()} className="bg-[#e95420] rounded px-4 py-2 text-sm cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed">
          Gönder
        </button>
      </div>
    </div>
  );
}
