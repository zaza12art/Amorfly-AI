import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import { openFileWithSystem } from '../lib/openFile';
import { logError } from '../lib/log';
import { Eye, PlayCircle, Loader2 } from 'lucide-react';

// ffmpeg pratikte tüm bu konteynerleri okuyabildiği için geniş tutuyoruz —
// "sadece birkaç format" sınırlaması kaldırıldı.
const VIDEO_EXTENSIONS = [
  'mp4', 'mkv', 'webm', 'avi', 'mov', 'flv', 'wmv', 'm4v', 'mpg', 'mpeg',
  '3gp', 'ts', 'm2ts', 'mts', 'vob', 'ogv', 'asf', 'rm', 'rmvb', 'divx',
];

export default function SubtitlesTab() {
  const [videoPath, setVideoPath] = useState<string | null>(null);
  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [previewFailed, setPreviewFailed] = useState(false);
  const [vttUrl, setVttUrl] = useState<string | null>(null);
  const [whisperBin, setWhisperBin] = useState('whisper-cli');
  const [whisperModel, setWhisperModel] = useState('');
  const [translationModel, setTranslationModel] = useState('llama3.2');
  const [piperBin, setPiperBin] = useState('piper');
  const [piperVoiceModel, setPiperVoiceModel] = useState('tr_TR-dfki-medium');

  const VOICE_KEY = 'voice_settings';
  useEffect(() => {
    invoke<string | null>('vault_read', { name: VOICE_KEY })
      .then((raw) => {
        if (!raw) return;
        try {
          const parsed = JSON.parse(raw);
          if (parsed.piperBin) setPiperBin(parsed.piperBin);
          if (parsed.piperVoiceModel) setPiperVoiceModel(parsed.piperVoiceModel);
          if (parsed.whisperBin) setWhisperBin(parsed.whisperBin);
          if (parsed.whisperModelPath) setWhisperModel(parsed.whisperModelPath);
        } catch {
          // ayarlar bozuk/okunamadı — varsayılanlarla devam
        }
      })
      .catch(() => {});
  }, []);
  const [translateEngine, setTranslateEngine] = useState<'ollama' | 'libretranslate'>('ollama');
  const [libretranslateUrl, setLibretranslateUrl] = useState('http://127.0.0.1:5000');
  const [vadModelPath, setVadModelPath] = useState('');
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dubbing, setDubbing] = useState(false);
  const [srtPath, setSrtPath] = useState<string | null>(null);
  const [dubOutputPath, setDubOutputPath] = useState<string | null>(null);

  const [pickerBusy, setPickerBusy] = useState(false);

  async function pickVideo(noFilter = false) {
    setPickerBusy(true);
    setError(null);
    try {
      const file = await open({
        multiple: false,
        filters: noFilter ? undefined : [{ name: 'Video (tüm formatlar)', extensions: VIDEO_EXTENSIONS }],
      });
      if (typeof file === 'string') {
        setVideoPath(file);
        // Not: videoUrl BİLEREK burada ayarlanmıyor. Dosya seçilir seçilmez
        // WebView'i büyük/egzotik codec'li bir videoyu önyüklemeye
        // zorlamak "donmuş" hissi yaratabiliyordu — önizleme artık
        // kullanıcı "Önizle" butonuna basınca, isteğe bağlı yükleniyor.
        setVideoUrl(null);
        setPreviewFailed(false);
        setVttUrl(null);
        setSrtPath(null);
        setDubOutputPath(null);
        setStatus(null);
      }
    } catch (e) {
      // Önceden burada hata yakalanmıyordu — dosya seçici herhangi bir
      // sebeple başarısız olursa (izin, WebView/portal sorunu vb.)
      // kullanıcıya HİÇBİR ŞEY görünmüyordu, tam olarak "donmuş" hissi
      // yaratıyordu. Artık en azından açık bir hata mesajı çıkıyor.
      setError(await logError('SubtitlesTab.pickVideo', e));
    } finally {
      setPickerBusy(false);
    }
  }

  function loadPreview() {
    if (videoPath) setVideoUrl(convertFileSrc(videoPath));
  }

  async function generateSubtitles() {
    if (!videoPath) return;
    setBusy(true);
    setError(null);
    setStatus('Ses çıkarılıyor…');
    // Canlı ilerleme: whisper transkripsiyonu sırasında "hâlâ çalışıyor"
    // sinyali, çeviri sırasında gerçek "X/Y segment" sayacı. Bu olmadan
    // uzun videolarda uygulama dakikalarca hiç güncellenmiyor gibi
    // görünüyordu — donmuş sanılıyordu.
    const unlisten = await listen<{ status: string; current: number; total: number; done: boolean }>(
      'amorfly://subtitle-progress',
      (event) => {
        if (!event.payload.done) setStatus(event.payload.status);
      }
    );
    try {
      const result = await invoke<{ srt_path: string; vtt_path: string; segment_count: number }>('generate_turkish_subtitles', {
        videoPath,
        whisperBin,
        whisperModelPath: whisperModel,
        translationModel,
        translateEngine,
        libretranslateUrl,
        vadModelPath,
      });
      setSrtPath(result.srt_path);
      setVttUrl(convertFileSrc(result.vtt_path));
      setStatus(
        `Tamamlandı — ${result.segment_count} segment Türkçe'ye çevrildi. ` +
        `.srt dosyası video ile aynı isimde kaydedildi (${result.srt_path}) — ` +
        `VLC/mpv gibi oynatıcılar bunu videoyu açtığında otomatik yükler.`
      );
    } catch (e) {
      setError(await logError('SubtitlesTab.generateSubtitles', e));
      setStatus(null);
    } finally {
      setBusy(false);
      unlisten();
    }
  }

  async function generateDub() {
    if (!videoPath || !srtPath) return;
    setDubbing(true);
    setError(null);
    setStatus('Türkçe seslendirme (Piper TTS) üretiliyor ve videoya mux ediliyor…');
    try {
      const outPath = await invoke<string>('generate_turkish_dub', {
        videoPath,
        srtPath,
        piperBin,
        piperVoiceModel,
      });
      setDubOutputPath(outPath);
      setStatus(`Dublajlı video oluşturuldu: ${outPath}`);
    } catch (e) {
      setError(await logError('SubtitlesTab.generateDub', e));
      setStatus(null);
    } finally {
      setDubbing(false);
    }
  }

  async function saveSrtAs() {
    if (!srtPath) return;
    const target = await save({ defaultPath: 'altyazi.srt', filters: [{ name: 'SubRip Altyazı', extensions: ['srt'] }] });
    if (!target) return;
    try {
      const { readTextFile, writeTextFile } = await import('@tauri-apps/plugin-fs');
      const content = await readTextFile(srtPath);
      await writeTextFile(target, content);
      setStatus(`Altyazı ayrıca şuraya kaydedildi: ${target}`);
    } catch (e) {
      setError(await logError('SubtitlesTab.saveSrtAs', e));
    }
  }

  async function openWithSystemPlayer(path: string) {
    try {
      await openFileWithSystem(path);
    } catch (e) {
      setError(await logError('SubtitlesTab.openWithSystemPlayer', e));
    }
  }

  return (
    <div className="max-w-3xl space-y-4">
      <p className="text-white/60 text-sm">
        Video aç → yerel whisper.cpp konuşmayı tanır (otomatik dil algılama, ~99 dil) → yerel Ollama modeli
        satır satır Türkçe'ye çevirir, hiçbir şeyi sansürlemez. Uygulama içindeki önizleme her video
        kodeğini gösteremeyebilir (WebView sınırlaması) — bu yüzden en güvenilir izleme yolu
        <strong> "Sistemde Aç"</strong> butonuyla kendi video oynatıcında (VLC/mpv vb.) açmaktır;
        altyazı dosyası video ile aynı klasörde aynı isimde olduğu için oynatıcı onu otomatik yükler.
      </p>

      <div className="bg-white/5 rounded-lg p-4 space-y-2 text-sm">
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
          <label className="flex flex-col gap-1">
            <span className="text-white/50">whisper.cpp binary</span>
            <input value={whisperBin} onChange={(e) => setWhisperBin(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1" />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-white/50">whisper model yolu (.bin)</span>
            <input value={whisperModel} onChange={(e) => setWhisperModel(e.target.value)} placeholder="/home/kullanici/whisper/ggml-small.bin" className="bg-white/10 text-white rounded px-2 py-1" />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-white/50">VAD modeli (opsiyonel, çok önerilir — hız için)</span>
            <input
              value={vadModelPath}
              onChange={(e) => setVadModelPath(e.target.value)}
              placeholder="/home/kullanici/whisper.cpp/models/ggml-silero-v6.2.0.bin"
              className="bg-white/10 rounded px-2 py-1"
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-white/50">Çeviri motoru</span>
            <select
              value={translateEngine}
              onChange={(e) => setTranslateEngine(e.target.value as 'ollama' | 'libretranslate')}
              className="bg-white/10 text-white rounded px-2 py-1 cursor-pointer"
            >
              <option value="ollama">Ollama (genel LLM, hazır kurulu)</option>
              <option value="libretranslate">LibreTranslate (özel çeviri motoru, ayrı kurulum ister)</option>
            </select>
          </label>
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          {translateEngine === 'ollama' ? (
            <label className="flex flex-col gap-1">
              <span className="text-white/50">çeviri modeli (Ollama)</span>
              <input value={translationModel} onChange={(e) => setTranslationModel(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1" />
            </label>
          ) : (
            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-white/50">
                LibreTranslate sunucu adresi — kurulu değilse: <code className="bg-black/30 px-1 rounded">docker run -ti -p 5000:5000 libretranslate/libretranslate</code>
              </span>
              <input value={libretranslateUrl} onChange={(e) => setLibretranslateUrl(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1" />
            </label>
          )}
        </div>
      </div>

      <div className="flex items-center gap-2">
        <button onClick={() => pickVideo(false)} disabled={pickerBusy} className="bg-[#e95420] rounded px-4 py-2 cursor-pointer disabled:opacity-50">
          {pickerBusy ? 'Dosya seçici açılıyor…' : 'Video Seç (tüm formatlar)'}
        </button>
        {!pickerBusy && (
          <button onClick={() => pickVideo(true)} className="text-white/40 text-xs underline cursor-pointer" title="Filtre listesi sorun çıkarırsa bunu dene">
            Filtresiz Seç
          </button>
        )}
        {videoPath && !videoUrl && (
          <button onClick={loadPreview} className="flex items-center gap-1 bg-white/10 rounded px-3 py-2 text-sm cursor-pointer">
            <Eye size={14} /> Önizle (isteğe bağlı)
          </button>
        )}
        {videoPath && (
          <button
            onClick={() => openWithSystemPlayer(videoPath)}
            className="flex items-center gap-1 bg-white/10 rounded px-3 py-2 text-sm cursor-pointer"
          >
            <PlayCircle size={14} /> Sistemde Aç
          </button>
        )}
      </div>

      {videoPath && (
        <div>
          <p className="text-xs text-white/40 mb-1 truncate">{videoPath}</p>
          {videoUrl && !previewFailed && (
            <video
              controls
              src={videoUrl}
              className="w-full rounded-lg bg-black max-h-80 mb-3"
              onError={() => setPreviewFailed(true)}
            >
              {vttUrl && <track kind="subtitles" src={vttUrl} srcLang="tr" label="Türkçe" default />}
            </video>
          )}
          {previewFailed && (
            <div className="bg-black/40 rounded-lg p-4 text-sm text-white/50 mb-3">
              Bu video formatı uygulama içinde önizlenemiyor (WebView'in kodek desteği sınırlı) —
              sorun değil, işleme devam edebilirsin. İzlemek için yukarıdaki "Sistemde Aç" butonunu kullan.
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <button onClick={generateSubtitles} disabled={busy} className="bg-[#e95420] rounded px-4 py-2 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer">
              {busy ? 'İşleniyor…' : 'Türkçe Altyazı Üret'}
            </button>
            {vttUrl && !videoUrl && (
              <button
                onClick={loadPreview}
                className="flex items-center gap-1 bg-[#e95420] rounded px-4 py-2 cursor-pointer"
                title="Videoyu uygulama içinde, Türkçe altyazılı olarak aç"
              >
                <Eye size={14} /> Altyazılı İzle
              </button>
            )}
            {srtPath && (
              <button onClick={generateDub} disabled={dubbing} className="bg-white/10 rounded px-4 py-2 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer">
                {dubbing ? 'Dublaj üretiliyor…' : 'Türkçe Dublaj Üret (opsiyonel)'}
              </button>
            )}
            {srtPath && (
              <button onClick={saveSrtAs} className="bg-white/10 rounded px-4 py-2 cursor-pointer">
                Altyazıyı Farklı Kaydet…
              </button>
            )}
            {dubOutputPath && (
              <button
                onClick={() => openWithSystemPlayer(dubOutputPath)}
                className="flex items-center gap-1 bg-white/10 rounded px-3 py-2 text-sm cursor-pointer"
              >
                <PlayCircle size={14} /> Dublajlı Videoyu Sistemde Aç
              </button>
            )}
          </div>
          {vttUrl && videoUrl && (
            <p className="text-xs text-[#e95420]/80 mt-1">
              Türkçe altyazı yukarıdaki oynatıcıya otomatik yüklendi (videonun altında/üstünde "CC" simgesiyle açıp kapatabilirsin).
            </p>
          )}
        </div>
      )}

      {status && (
        <p className="text-white/60 text-sm flex items-center gap-2">
          {busy && <Loader2 size={14} className="animate-spin shrink-0" />}
          {status}
        </p>
      )}
      {error && <p className="text-red-400 text-sm bg-red-950/30 rounded p-3">{error}</p>}

      <div className="text-xs text-white/40 bg-white/5 rounded p-3">
        Gerekli kurulumlar (bu program bunları paketlemez, GB'larca dosya içerdikleri için):
        <br />• ffmpeg — <code className="bg-black/30 px-1 rounded">sudo apt install ffmpeg</code>
        <br />• whisper.cpp — github.com/ggerganov/whisper.cpp (derleyip bir ggml model indirin)
        <br />• <strong>VAD modeli (hız için çok önerilir)</strong> — whisper.cpp klasöründe:{' '}
        <code className="bg-black/30 px-1 rounded">./models/download-vad-model.sh silero-v6.2.0</code>{' '}
        (~1MB, saniyeler içinde iner). Bu, whisper.cpp'nin sessiz kısımları atlayıp SADECE konuşmayı
        işlemesini sağlar — uzun videolarda ciddi hız kazandırır.
        <br />• (opsiyonel dublaj) Piper TTS + Türkçe ses modeli — Ayarlar sekmesinden otomatik kurulabilir
      </div>
    </div>
  );
}
