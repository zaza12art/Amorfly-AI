import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { logError } from '../lib/log';
import { FolderSearch, ScanText, Sparkles, X, Bot, Play } from 'lucide-react';

interface Task {
  id: string;
  kind: string;
  title: string;
  status: string;
  progress: number;
  log: string[];
  result: string | null;
}

interface PlanStep {
  id: string;
  label: string;
}

export default function TasksTab() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [error, setError] = useState<string | null>(null);

  const [goal, setGoal] = useState('');
  const [plan, setPlan] = useState<PlanStep[] | null>(null);
  const [planning, setPlanning] = useState(false);
  const [agentVideoPath, setAgentVideoPath] = useState<string | null>(null);
  const [launchingWorkflow, setLaunchingWorkflow] = useState(false);

  useEffect(() => {
    invoke<Task[]>('list_tasks').then(setTasks).catch(() => {});
    const unlisten = listen<Task[]>('amorfly://tasks-updated', (e) => setTasks(e.payload));
    return () => { unlisten.then((f) => f()); };
  }, []);

  async function scanFolder() {
    const dir = await open({ directory: true });
    if (typeof dir !== 'string') return;
    try {
      await invoke('queue_folder_scan', { folderPath: dir });
    } catch (e) {
      setError(await logError('TasksTab.scanFolder', e));
    }
  }

  async function batchOcr() {
    const dir = await open({ directory: true });
    if (typeof dir !== 'string') return;
    const ok = window.confirm(
      `'${dir}' klasöründeki (alt klasörler dahil) tüm PDF'ler için OCR yapılacak.\n` +
      `Orijinal PDF'ler değişmez, her biri için yanına bir .ocr.txt dosyası eklenir.\n\nDevam edilsin mi?`
    );
    if (!ok) return;
    try {
      await invoke('queue_batch_ocr', { folderPath: dir, confirmed: true });
    } catch (e) {
      setError(await logError('TasksTab.batchOcr', e));
    }
  }

  async function batchUpscale() {
    const dir = await open({ directory: true });
    if (typeof dir !== 'string') return;
    const ok = window.confirm(
      `'${dir}' klasöründeki (alt klasörler dahil) tüm videolar 2x büyütülecek.\n` +
      `Orijinal videolar değişmez, her biri için ayrı bir .upscaled.mp4 dosyası oluşturulur.\n` +
      `Bu işlem çok uzun sürebilir (saatler). Devam edilsin mi?`
    );
    if (!ok) return;
    try {
      await invoke('queue_batch_upscale', { folderPath: dir, scale: 2, model: 'RealESRGAN_x4plus', confirmed: true });
    } catch (e) {
      setError(await logError('TasksTab.batchUpscale', e));
    }
  }

  async function cancelTask(id: string) {
    await invoke('cancel_task', { id });
  }

  async function makePlan() {
    if (!goal.trim()) return;
    setPlanning(true);
    setPlan(null);
    setError(null);
    try {
      const steps = await invoke<PlanStep[]>('plan_workflow', { goal, model: '' });
      setPlan(steps);
    } catch (e) {
      setError(await logError('TasksTab.makePlan', e));
    } finally {
      setPlanning(false);
    }
  }

  async function pickAgentVideo() {
    const file = await open({
      multiple: false,
      filters: [{ name: 'Video', extensions: ['mp4', 'mkv', 'webm', 'avi', 'mov'] }],
    });
    if (typeof file === 'string') setAgentVideoPath(file);
  }

  async function runPlan() {
    if (!plan || !agentVideoPath) return;
    const stepLabels = plan.map((s) => `• ${s.label}`).join('\n');
    const ok = window.confirm(`Şu adımlar sırayla çalıştırılacak:\n\n${stepLabels}\n\nDevam edilsin mi?`);
    if (!ok) return;

    setLaunchingWorkflow(true);
    try {
      await invoke('run_workflow', {
        inputPath: agentVideoPath,
        stepIds: plan.map((s) => s.id),
        upscaleModel: 'RealESRGAN_x4plus',
        translationModel: 'llama3.2',
        piperVoiceModel: 'tr_TR-dfki-medium',
        whisperBin: 'whisper-cli',
        whisperModelPath: '',
        confirmed: true,
      });
      setPlan(null);
      setGoal('');
      setAgentVideoPath(null);
    } catch (e) {
      setError(await logError('TasksTab.runPlan', e));
    } finally {
      setLaunchingWorkflow(false);
    }
  }

  const statusColor: Record<string, string> = {
    'kuyrukta': 'text-white/40',
    'çalışıyor': 'text-yellow-400',
    'tamamlandı': 'text-green-400',
    'hata': 'text-red-400',
    'iptal edildi': 'text-white/30',
  };

  return (
    <div className="max-w-3xl space-y-5">
      <p className="text-white/60 text-sm">
        Uzun sürecek toplu işleri buradan kuyruğa al — sekmeyi değiştirsen bile arka planda devam eder.
        Toplu/kalıcı-etkili işler önce bir onay penceresi gösterir, sen "evet" demeden çalışmaz.
      </p>

      <div className="bg-white/5 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-2">
          <Bot size={16} className="text-[#e95420]" />
          <p className="text-sm font-medium">Otomatik İş Akışı (Agent)</p>
        </div>
        <p className="text-xs text-white/40 mb-3">
          Tek cümleyle ne istediğini yaz — yerel model bunu bilinen adımlara (altyazı, dublaj,
          kalite artırma) çevirir. Sana planı gösterir, sen onaylamadan hiçbir şey çalışmaz.
        </p>
        <div className="flex gap-2 mb-3">
          <input
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && makePlan()}
            placeholder="Bu videoyu Türkçeye çevir, dublaj yap ve 4K yap"
            className="flex-1 bg-white/10 text-white rounded px-3 py-2 text-sm outline-none"
          />
          <button onClick={makePlan} disabled={planning} className="bg-white/10 rounded px-4 py-2 text-sm disabled:opacity-40 cursor-pointer">
            {planning ? 'Planlanıyor…' : 'Plan Oluştur'}
          </button>
        </div>

        {plan && (
          <div className="bg-black/20 rounded-lg p-3">
            <p className="text-xs text-white/50 mb-2">Önerilen adımlar:</p>
            <ol className="text-sm space-y-1 mb-3 list-decimal list-inside">
              {plan.map((s) => <li key={s.id}>{s.label}</li>)}
            </ol>

            <div className="flex items-center gap-2">
              <button onClick={pickAgentVideo} className="bg-white/10 rounded px-3 py-1.5 text-xs cursor-pointer">
                {agentVideoPath ? agentVideoPath.split('/').pop() : 'Video Seç'}
              </button>
              <button
                onClick={runPlan}
                disabled={!agentVideoPath || launchingWorkflow}
                className="flex items-center gap-1 bg-[#e95420] rounded px-3 py-1.5 text-xs disabled:opacity-40 cursor-pointer"
              >
                <Play size={12} /> {launchingWorkflow ? 'Başlatılıyor…' : 'Onayla ve Çalıştır'}
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <button onClick={scanFolder} className="bg-white/5 hover:bg-white/10 rounded-lg p-4 text-left cursor-pointer">
          <FolderSearch size={18} className="mb-2 text-[#e95420]" />
          <p className="text-sm font-medium">Klasör Tara</p>
          <p className="text-xs text-white/40 mt-1">Dosya/tür/boyut raporu — salt okunur, onay gerekmez</p>
        </button>
        <button onClick={batchOcr} className="bg-white/5 hover:bg-white/10 rounded-lg p-4 text-left cursor-pointer">
          <ScanText size={18} className="mb-2 text-[#e95420]" />
          <p className="text-sm font-medium">Toplu OCR (PDF)</p>
          <p className="text-xs text-white/40 mt-1">tesseract-ocr ile klasördeki tüm PDF'leri metne çevir</p>
        </button>
        <button onClick={batchUpscale} className="bg-white/5 hover:bg-white/10 rounded-lg p-4 text-left cursor-pointer">
          <Sparkles size={18} className="mb-2 text-[#e95420]" />
          <p className="text-sm font-medium">Toplu Kalite Artırma</p>
          <p className="text-xs text-white/40 mt-1">Video2X ile klasördeki tüm videoları büyüt</p>
        </button>
      </div>

      {error && <p className="text-red-400 text-sm bg-red-950/30 rounded p-3">{error}</p>}

      <div className="space-y-3">
        {tasks.length === 0 && <p className="text-white/30 text-sm">Henüz görev yok.</p>}
        {[...tasks].reverse().map((t) => (
          <div key={t.id} className="bg-white/5 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <p className="text-sm font-medium">{t.title}</p>
              <div className="flex items-center gap-2">
                <span className={'text-xs ' + (statusColor[t.status] || 'text-white/40')}>{t.status}</span>
                {t.status === 'çalışıyor' && (
                  <button onClick={() => cancelTask(t.id)} className="text-white/30 hover:text-red-400 cursor-pointer" title="İptal et">
                    <X size={14} />
                  </button>
                )}
              </div>
            </div>
            <div className="w-full bg-black/30 rounded h-1.5 mt-2 overflow-hidden">
              <div className="bg-[#e95420] h-full" style={{ width: `${t.progress}%` }} />
            </div>
            {t.result && <p className="text-xs text-white/50 mt-2 whitespace-pre-wrap">{t.result}</p>}
            {t.log.length > 0 && (
              <details className="mt-2">
                <summary className="text-xs text-white/30 cursor-pointer">Ayrıntılar ({t.log.length})</summary>
                <pre className="text-xs text-white/40 mt-1 max-h-32 overflow-y-auto whitespace-pre-wrap">{t.log.join('\n')}</pre>
              </details>
            )}
          </div>
        ))}
      </div>

      <div className="text-xs text-white/40 bg-white/5 rounded p-3">
        Toplu OCR için gerekli: <code className="bg-black/30 px-1 rounded">tesseract-ocr</code> (sudo apt install tesseract-ocr tesseract-ocr-tur).
        Toplu kalite artırma için Video2X gerekir (Tanılama sekmesinden kurulabilir).
      </div>
    </div>
  );
}
