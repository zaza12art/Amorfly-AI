import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { openFileWithSystem } from '../lib/openFile';
import { logError } from '../lib/log';
import { copyToClipboard } from '../lib/clipboard';
import { PlayCircle, Eye, Loader2 } from 'lucide-react';

/** Vulkan bulunamadığında GPU markasına göre doğru terminal komutunu
 * önerir. BİLEREK otomatik kurulum yapmıyoruz — yanlış/uyumsuz bir GPU
 * sürücüsü ekranı bozabilir, bu riski kullanıcı adına almıyoruz. Sadece
 * doğru komutu gösterip kopyalatıyoruz, çalıştırmak kullanıcının kararı.
 *
 * GERÇEK OLAY NOTU: Bir kullanıcıda "ubuntu-drivers autoinstall" sonrası
 * WiFi çalışmaz hale geldi ve açılış ekranda takıldı. Bu teorik değil,
 * gerçekten yaşandı — bu yüzden uyarı metni artık çok daha güçlü ve
 * ÖNCE-KONTROL-ET / SONRA-KUR şeklinde iki adıma bölündü, kör bir
 * "autoinstall" yerine.
 */
function VulkanHelp({ vendorInfo }: { vendorInfo: { vendor: string; model: string } }) {
  const haystack = `${vendorInfo.vendor} ${vendorInfo.model}`.toLowerCase();
  const isNvidia = haystack.includes('nvidia');
  const isAmd = haystack.includes('amd') || haystack.includes('radeon') || haystack.includes('ati ');
  const isIntel = haystack.includes('intel');

  let command = 'sudo apt install -y mesa-vulkan-drivers vulkan-tools';
  let checkCommand: string | null = null;
  let note = 'Genel Vulkan sürücüsü (çoğu Intel/AMD entegre/harici kart için). Bu, sistem çekirdeğini etkilemez, düşük risklidir.';

  if (isNvidia) {
    checkCommand = 'ubuntu-drivers devices';
    command = 'sudo ubuntu-drivers autoinstall && sudo apt install -y vulkan-tools';
    note = 'NVIDIA kartın var. NVIDIA sürücüleri çekirdek modülü değiştirdiği için DAHA RİSKLİ — bazı sistemlerde WiFi/ağ sürücüsüyle çakışıp açılışın takılmasına yol açabiliyor (bu gerçekten yaşandı).';
  } else if (isAmd) {
    command = 'sudo apt install -y mesa-vulkan-drivers vulkan-tools';
    note = 'AMD/Radeon kartın var — açık kaynak Mesa Vulkan sürücüsü genelde yeterli, ekstra bir şey gerekmez. Düşük risklidir.';
  } else if (isIntel) {
    command = 'sudo apt install -y mesa-vulkan-drivers intel-media-va-driver vulkan-tools';
    note = 'Intel entegre grafik kartın var — Mesa Vulkan sürücüsü genelde yeterli. Düşük risklidir.';
  }

  return (
    <div className="mt-2 bg-[#e95420]/10 border border-[#e95420]/30 rounded-lg p-3 text-xs">
      <p className="text-white/70 mb-2">
        Vulkan sürücüsü bulunamadı. {note}
      </p>
      {isNvidia && (
        <div className="bg-red-950/40 border border-red-500/30 rounded p-2 mb-2">
          <p className="text-red-300 font-medium mb-1">Önce yedek al:</p>
          <p className="text-white/60">
            Timeshift ile bir sistem geri yükleme noktası oluştur (Zorin'de hazır kurulu genelde:
            uygulama menüsünden "Timeshift" ara). Bir şeyler ters giderse birkaç dakikada eski
            haline dönebilirsin. NVIDIA sürücüsü kurulumu, hiç yedek almadan ilerlenecek bir
            işlem değil.
          </p>
        </div>
      )}
      {checkCommand && (
        <div className="mb-2">
          <p className="text-white/40 mb-1">1) Önce sadece hangi sürücülerin önerildiğini gör (henüz bir şey kurmaz):</p>
          <div className="flex items-center gap-2">
            <code className="flex-1 bg-black/40 rounded px-2 py-1 overflow-x-auto whitespace-nowrap">{checkCommand}</code>
            <button
              onClick={() => copyToClipboard(checkCommand!)}
              className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-2 py-1 cursor-pointer"
            >
              Kopyala
            </button>
          </div>
        </div>
      )}
      <p className="text-white/40 mb-1">
        {checkCommand ? '2) Yedek aldıktan ve önerilen sürücüyü gördükten sonra, emin isen:' : 'Otomatik kurmuyoruz, kararı sana bırakıyoruz. Emin isen terminale yapıştır:'}
      </p>
      <div className="flex items-center gap-2">
        <code className="flex-1 bg-black/40 rounded px-2 py-1 overflow-x-auto whitespace-nowrap">{command}</code>
        <button
          onClick={() => copyToClipboard(command)}
          className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-2 py-1 cursor-pointer"
        >
          Kopyala
        </button>
      </div>
      <p className="text-white/30 mt-1">Tespit edilen donanım: {vendorInfo.model || vendorInfo.vendor}</p>
    </div>
  );
}

interface GpuOption {
  index: number;
  name: string;
}

const MODELS = [
  { id: 'RealESRGAN_x4plus', label: 'Gerçek video (kamera, telefon, VHS)' },
  { id: 'realesr-animevideov3', label: 'Anime / çizim içerik' },
  { id: 'realesrgan-plus-anime', label: 'Hibrit (gerçek + animasyon karışık)' },
];

// ffmpeg/Video2X pratikte tüm bu konteynerleri işleyebiliyor.
const VIDEO_EXTENSIONS = [
  'mp4', 'mkv', 'webm', 'avi', 'mov', 'flv', 'wmv', 'm4v', 'mpg', 'mpeg',
  '3gp', 'ts', 'm2ts', 'mts', 'vob', 'ogv', 'asf', 'rm', 'rmvb', 'divx',
];

export default function UpscaleTab() {
  const [videoPath, setVideoPath] = useState<string | null>(null);
  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [previewFailed, setPreviewFailed] = useState(false);
  const [gpus, setGpus] = useState<GpuOption[]>([]);
  const [selectedGpu, setSelectedGpu] = useState<number | undefined>(undefined);
  const [scale, setScale] = useState(2);
  const [model, setModel] = useState(MODELS[0].id);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [resultPath, setResultPath] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [gpuVendorInfo, setGpuVendorInfo] = useState<{ vendor: string; model: string } | null>(null);
  const [video2xMissing, setVideo2xMissing] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);

  useEffect(() => {
    loadGpus();
    invoke<{ vendor: string; model: string }>('get_gpu_info').then(setGpuVendorInfo).catch(() => {});
  }, []);

  function loadGpus() {
    invoke<GpuOption[]>('list_upscale_gpus')
      .then((list) => {
        setGpus(list);
        setVideo2xMissing(false);
        if (list.length > 0) setSelectedGpu(list[0].index);
      })
      .catch(() => setVideo2xMissing(true));
  }

  async function installVideo2x() {
    setInstalling(true);
    setInstallError(null);
    try {
      await invoke('install_video2x_portable');
      loadGpus();
    } catch (e) {
      setInstallError(await logError('UpscaleTab.installVideo2x', e));
    } finally {
      setInstalling(false);
    }
  }

  useEffect(() => {
    if (!busy) return;
    const start = Date.now();
    const t = setInterval(() => setElapsed(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => clearInterval(t);
  }, [busy]);

  const [pickerBusy, setPickerBusy] = useState(false);

  async function pickVideo(noFilter = false) {
    // Not: dosya boyutuna kasıtlı olarak hiçbir sınır konmuyor.
    setPickerBusy(true);
    setError(null);
    try {
      const file = await open({
        multiple: false,
        filters: noFilter ? undefined : [{ name: 'Video (tüm formatlar)', extensions: VIDEO_EXTENSIONS }],
      });
      if (typeof file === 'string') {
        setVideoPath(file);
        // videoUrl bilerek burada ayarlanmıyor — WebView'i büyük/egzotik
        // codec'li bir videoyu otomatik önyüklemeye zorlamak donma hissi
        // yaratıyordu. Önizleme artık "Önizle" butonuna basınca yükleniyor.
        setVideoUrl(null);
        setPreviewFailed(false);
        setResultPath(null);
        setStatus(null);
      }
    } catch (e) {
      // Önceden burada hata yakalanmıyordu — dosya seçici herhangi bir
      // sebeple başarısız olursa kullanıcıya hiçbir şey görünmüyordu,
      // "donmuş" hissi yaratıyordu. Artık açık bir hata mesajı çıkıyor.
      setError(await logError('UpscaleTab.pickVideo', e));
    } finally {
      setPickerBusy(false);
    }
  }

  function loadPreview() {
    if (videoPath) setVideoUrl(convertFileSrc(videoPath));
  }

  async function runUpscale() {
    if (!videoPath) return;
    setBusy(true);
    setError(null);
    setElapsed(0);
    setStatus('Video2X başlatılıyor…');
    // Gerçek backend heartbeat'i dinle — video2x'in kendisi çok uzun
    // sürebildiği için (donanıma göre saatler), düzenli "hâlâ çalışıyor"
    // sinyali olmadan uygulama donmuş gibi görünüyordu.
    const unlisten = await listen<{ status: string; elapsed_secs: number; done: boolean }>(
      'amorfly://upscale-progress',
      (event) => {
        if (!event.payload.done) setStatus(event.payload.status);
      }
    );
    try {
      const result = await invoke<{ output_path: string }>('upscale_video', {
        videoPath,
        scale,
        model,
        gpuIndex: selectedGpu,
      });
      setResultPath(result.output_path);
      setStatus(`Tamamlandı (${elapsed}s) — çıktı: ${result.output_path}`);
    } catch (e) {
      setError(await logError('UpscaleTab.runUpscale', e));
      setStatus(null);
    } finally {
      setBusy(false);
      unlisten();
    }
  }

  async function openWithSystemPlayer(path: string) {
    try {
      await openFileWithSystem(path);
    } catch (e) {
      setError(await logError('UpscaleTab.openWithSystemPlayer', e));
    }
  }

  return (
    <div className="max-w-3xl space-y-4">
      <p className="text-white/60 text-sm">
        Video2X (Real-ESRGAN / Real-CUGAN / Anime4K) ile eski, düşük çözünürlüklü videoları
        FHD ya da 4K'ya büyütür — tamamen yerel, dosya boyutuna sınır yok. Video ne kadar
        büyük/uzun olursa olsun işlenir, sadece süre uzar.
      </p>

      <div className="bg-white/5 rounded-lg p-4 space-y-3 text-sm">
        {video2xMissing && (
          <div className="bg-[#e95420]/10 border border-[#e95420]/30 rounded-lg p-3">
            <p className="mb-2">Video2X kurulu değil.</p>
            <button onClick={installVideo2x} disabled={installing} className="bg-[#e95420] rounded px-3 py-1.5 text-xs disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer">
              {installing ? 'İndiriliyor…' : "Video2X'i Otomatik Kur"}
            </button>
            <p className="text-white/30 text-xs mt-1">GitHub'dan resmi AppImage indirilir, hesap/kayıt gerekmez.</p>
            {installError && <p className="text-red-400 text-xs mt-1">{installError}</p>}
            <div className="mt-2 bg-black/30 rounded p-2">
              <p className="text-white/40 text-xs mb-1">Otomatik kurulum çalışmazsa, terminale yapıştır (tüm motorları tek seferde kontrol edip kurar):</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 bg-black/40 rounded px-2 py-1 text-xs text-white/70 overflow-x-auto whitespace-nowrap">
                  curl -fsSL https://raw.githubusercontent.com/zaza12art/Amorfly-AI/main/motorlari_elle_kur.sh | bash
                </code>
                <button
                  onClick={() => copyToClipboard('curl -fsSL https://raw.githubusercontent.com/zaza12art/Amorfly-AI/main/motorlari_elle_kur.sh | bash')}
                  className="shrink-0 bg-white/10 hover:bg-white/20 rounded px-2 py-1 text-xs cursor-pointer"
                >
                  Kopyala
                </button>
              </div>
            </div>
          </div>
        )}
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          <label className="flex flex-col gap-1">
            <span className="text-white/50">Büyütme oranı</span>
            <select value={scale} onChange={(e) => setScale(Number(e.target.value))} className="bg-white/10 text-white rounded px-2 py-1 cursor-pointer">
              <option value={2}>2x (ör. FHD → 4K'ya yakın)</option>
              <option value={4}>4x (ör. 480p → 4K)</option>
            </select>
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-white/50">İçerik türü</span>
            <select value={model} onChange={(e) => setModel(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1 cursor-pointer">
              {MODELS.map((m) => <option key={m.id} value={m.id}>{m.label}</option>)}
            </select>
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-white/50">GPU</span>
            {gpus.length > 0 ? (
              <select value={selectedGpu} onChange={(e) => setSelectedGpu(Number(e.target.value))} className="bg-white/10 text-white rounded px-2 py-1 cursor-pointer">
                {gpus.map((g) => <option key={g.index} value={g.index}>{g.name}</option>)}
              </select>
            ) : (
              <span className="text-white/30 text-xs py-1">Vulkan GPU bulunamadı</span>
            )}
          </label>
        </div>
        {gpus.length === 0 && gpuVendorInfo && (
          <VulkanHelp vendorInfo={gpuVendorInfo} />
        )}
      </div>

      <div className="flex items-center gap-2">
        <button onClick={() => pickVideo()} className="bg-[#e95420] rounded px-4 py-2 cursor-pointer">Video Seç (tüm formatlar)</button>
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
              className="w-full rounded-lg bg-black max-h-80"
              onError={() => setPreviewFailed(true)}
            />
          )}
          {previewFailed && (
            <div className="bg-black/40 rounded-lg p-4 text-sm text-white/50">
              Bu video formatı uygulama içinde önizlenemiyor (WebView'in kodek desteği sınırlı) —
              sorun değil, işleme devam edebilirsin. İzlemek için yukarıdaki "Sistemde Aç" butonunu kullan.
            </div>
          )}
          <button onClick={runUpscale} disabled={busy} className="bg-[#e95420] rounded px-4 py-2 mt-3 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer">
            {busy ? `İşleniyor… (${elapsed}s)` : 'Kaliteyi Artır'}
          </button>
        </div>
      )}

      {status && (
        <p className="text-white/60 text-sm flex items-center gap-2">
          {busy && <Loader2 size={14} className="animate-spin shrink-0" />}
          {status}
        </p>
      )}
      {error && <p className="text-red-400 text-sm bg-red-950/30 rounded p-3">{error}</p>}

      {resultPath && (
        <div>
          <p className="text-sm text-white/50 mb-2 flex items-center justify-between">
            <span>Sonuç: {resultPath}</span>
            <button
              onClick={() => openWithSystemPlayer(resultPath)}
              className="flex items-center gap-1 bg-white/10 rounded px-3 py-1.5 text-xs cursor-pointer"
            >
              <PlayCircle size={14} /> Sistemde Aç
            </button>
          </p>
        </div>
      )}

      <div className="text-xs text-white/40 bg-white/5 rounded p-3">
        Gerekli kurulum: <code className="bg-black/30 px-1 rounded">video2x</code> —
        github.com/k4yt3x/video2x (Linux AppImage). Vulkan destekli bir GPU gerekiyor
        (NVIDIA/AMD/Intel fark etmez). Kare hızı (fps) artırmak istersen (eski, takılan
        videolar için) RIFE motoru ayrı bir işlem — istersen bunu da ekleyebilirim.
      </div>
    </div>
  );
}
