import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { openFileWithSystem } from '../lib/openFile';
import { FileText, Image as ImageIcon, Download, Search, Database, Table, Sparkles } from 'lucide-react';
import MessageContent from '../components/MessageContent';
import { logError } from '../lib/log';

const DOC_EXTENSIONS = ['xlsx', 'xls', 'xlsm', 'pdf', 'docx', 'doc', 'odt', 'txt', 'md', 'csv'];
const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp'];

const EXPORT_FORMATS = [
  { id: 'txt', label: 'Metin (.txt)' },
  { id: 'docx', label: 'Word (.docx)' },
  { id: 'pdf', label: 'PDF (.pdf)' },
  { id: 'xlsx', label: 'Excel (.xlsx)' },
];

interface SearchHit {
  path: string;
  snippet: string;
  score: number;
}

function extOf(path: string) {
  return path.split('.').pop()?.toLowerCase() || '';
}

interface DocumentsTabProps {
  workMode: 'lokal' | 'hibrit' | 'online';
}

export default function DocumentsTab({ workMode }: DocumentsTabProps) {
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileType, setFileType] = useState<'document' | 'image' | null>(null);
  const [question, setQuestion] = useState('');
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [result, setResult] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exportFormat, setExportFormat] = useState('txt');
  const [exporting, setExporting] = useState(false);
  const [exportedPath, setExportedPath] = useState<string | null>(null);

  const [indexing, setIndexing] = useState(false);
  const [indexedCount, setIndexedCount] = useState(0);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);

  // Akıllı Excel Üretici
  const [excelDescription, setExcelDescription] = useState(() => {
    const pending = sessionStorage.getItem('pending_excel_description');
    if (pending) sessionStorage.removeItem('pending_excel_description');
    return pending ?? '';
  });
  const [excelBusy, setExcelBusy] = useState<'ai' | 'steel' | 'rebar' | null>(null);
  const [excelResultPath, setExcelResultPath] = useState<string | null>(null);
  const [excelError, setExcelError] = useState<string | null>(null);

  async function generateAiExcel() {
    if (!excelDescription.trim()) return;
    const outPath = await save({ defaultPath: 'tablo.xlsx', filters: [{ name: 'Excel', extensions: ['xlsx'] }] });
    if (!outPath) return;
    setExcelBusy('ai');
    setExcelError(null);
    setExcelResultPath(null);
    try {
      const path = await invoke<string>('generate_excel_from_description', {
        description: excelDescription,
        model: selectedModel,
        outputPath: outPath,
        mode: workMode,
      });
      setExcelResultPath(path);
    } catch (e) {
      setExcelError(await logError('DocumentsTab.generateAiExcel', e));
    } finally {
      setExcelBusy(null);
    }
  }

  async function exportSteelProfiles() {
    const outPath = await save({ defaultPath: 'celik-profilleri.xlsx', filters: [{ name: 'Excel', extensions: ['xlsx'] }] });
    if (!outPath) return;
    setExcelBusy('steel');
    setExcelError(null);
    setExcelResultPath(null);
    try {
      const path = await invoke<string>('export_steel_profiles_excel', { outputPath: outPath });
      setExcelResultPath(path);
    } catch (e) {
      setExcelError(await logError('DocumentsTab.exportSteelProfiles', e));
    } finally {
      setExcelBusy(null);
    }
  }

  async function exportRebarTable() {
    const outPath = await save({ defaultPath: 'donati-agirlik-tablosu.xlsx', filters: [{ name: 'Excel', extensions: ['xlsx'] }] });
    if (!outPath) return;
    setExcelBusy('rebar');
    setExcelError(null);
    setExcelResultPath(null);
    try {
      const path = await invoke<string>('export_rebar_table_excel', { outputPath: outPath });
      setExcelResultPath(path);
    } catch (e) {
      setExcelError(await logError('DocumentsTab.exportRebarTable', e));
    } finally {
      setExcelBusy(null);
    }
  }

  useEffect(() => {
    invoke<string[]>('list_ollama_models').then((list) => {
      setModels(list);
      if (list.length > 0) {
        invoke<string>('suggest_model_for_task', { task: 'belge_analiz', installedModels: list })
          .then(setSelectedModel)
          .catch(() => setSelectedModel(list[0]));
      }
    }).catch(() => {});
    invoke<number>('indexed_document_count').then(setIndexedCount).catch(() => {});
  }, []);

  async function indexCurrentFile() {
    if (!filePath || fileType === 'image') return;
    setIndexing(true);
    try {
      await invoke<number>('index_document', { path: filePath, embedModel: 'nomic-embed-text' });
      const count = await invoke<number>('indexed_document_count');
      setIndexedCount(count);
    } catch (e) {
      setError(await logError('DocumentsTab.indexCurrentFile', e));
    } finally {
      setIndexing(false);
    }
  }

  async function searchPastDocuments() {
    if (!searchQuery.trim()) return;
    setSearching(true);
    setSearchResults([]);
    try {
      const hits = await invoke<SearchHit[]>('search_documents', { query: searchQuery, embedModel: 'nomic-embed-text', topK: 5 });
      setSearchResults(hits);
    } catch (e) {
      setError(await logError('DocumentsTab.searchPastDocuments', e));
    } finally {
      setSearching(false);
    }
  }

  async function pickFile() {
    const file = await open({
      multiple: false,
      filters: [
        { name: 'Belgeler', extensions: DOC_EXTENSIONS },
        { name: 'Görseller', extensions: IMAGE_EXTENSIONS },
      ],
    });
    if (typeof file === 'string') {
      setFilePath(file);
      const ext = extOf(file);
      setFileType(IMAGE_EXTENSIONS.includes(ext) ? 'image' : 'document');
      setResult('');
      setExportedPath(null);
      setError(null);
    }
  }

  async function runAnalysis() {
    if (!filePath || !question.trim()) return;
    setBusy(true);
    setError(null);
    setResult('');
    setExportedPath(null);
    try {
      const output = fileType === 'image'
        ? await invoke<string>('analyze_image', { path: filePath, question, model: selectedModel })
        : await invoke<string>('analyze_document', { path: filePath, question, model: selectedModel, mode: workMode });
      setResult(output);
    } catch (e) {
      setError(await logError('DocumentsTab.runAnalysis', e));
    } finally {
      setBusy(false);
    }
  }

  async function exportResult() {
    if (!result) return;
    const defaultName = `amorfly_analiz.${exportFormat}`;
    const path = await save({ defaultPath: defaultName });
    if (!path) return;

    setExporting(true);
    setError(null);
    try {
      const finalPath = await invoke<string>('export_document', { content: result, format: exportFormat, outputPath: path });
      setExportedPath(finalPath);
    } catch (e) {
      setError(await logError('DocumentsTab.exportResult', e));
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="max-w-3xl space-y-4">
      <p className="text-white/60 text-sm">
        Excel, PDF, Word ya da görsel yükle; ne istediğini yaz (özetle, analiz et, veriyi çıkar, vb.).
        Yerel modelin cevabını istersen .txt, .docx, .pdf ya da .xlsx olarak kaydedebilirsin.
      </p>

      <div className="bg-white/5 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-1">
          <Table size={14} className="text-[#e95420]" />
          <p className="text-sm font-medium">Akıllı Excel Üretici</p>
        </div>
        <p className="text-xs text-white/40 mb-3">
          Ne istediğini yaz, gerçek formüllü (statik sayı değil) bir .xlsx üretilsin. Ör: "donatı
          hesabı yapabileceğim, çap girince ağırlığı otomatik hesaplayan bir tablo".
        </p>
        <div className="flex gap-2 mb-3">
          <input
            value={excelDescription}
            onChange={(e) => setExcelDescription(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && generateAiExcel()}
            placeholder="Ne tür bir Excel tablosu istiyorsun?"
            className="flex-1 bg-white/10 text-white rounded px-3 py-2 text-sm outline-none"
          />
          <button
            onClick={generateAiExcel}
            disabled={!excelDescription.trim() || excelBusy !== null}
            className="bg-[#e95420] rounded px-3 py-2 text-sm flex items-center gap-1 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
          >
            <Sparkles size={14} /> {excelBusy === 'ai' ? 'Üretiliyor…' : 'Excel Oluştur'}
          </button>
        </div>

        <p className="text-xs text-white/30 mb-2">Hazır, doğrulanmış mühendislik referans tabloları:</p>
        <div className="flex flex-wrap gap-2">
          <button
            onClick={exportSteelProfiles}
            disabled={excelBusy !== null}
            className="bg-white/10 rounded px-3 py-1.5 text-xs disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
          >
            {excelBusy === 'steel' ? 'Hazırlanıyor…' : 'Çelik Profilleri (IPE/INP/HEA/HEB/UNP/UPE)'}
          </button>
          <button
            onClick={exportRebarTable}
            disabled={excelBusy !== null}
            className="bg-white/10 rounded px-3 py-1.5 text-xs disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
          >
            {excelBusy === 'rebar' ? 'Hazırlanıyor…' : 'Donatı Ağırlık Tablosu'}
          </button>
        </div>

        {excelResultPath && (
          <div className="mt-3 bg-black/20 rounded p-2 text-xs flex items-center justify-between gap-2">
            <span className="text-white/60 truncate">{excelResultPath}</span>
            <button onClick={() => openFileWithSystem(excelResultPath)} className="shrink-0 bg-white/10 rounded px-2 py-1 cursor-pointer">
              Aç
            </button>
          </div>
        )}
        {excelError && <p className="text-red-400 text-xs mt-2">{excelError}</p>}
      </div>

      <div className="bg-white/5 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-1">
          <Database size={14} className="text-[#e95420]" />
          <p className="text-sm font-medium">Belge Hafızasında Ara</p>
          <span className="text-xs text-white/30">({indexedCount} belge indekslenmiş)</span>
        </div>
        <p className="text-xs text-white/40 mb-2">
          Daha önce indekslediğin belgelerde anlamsal arama yap — ör. "geçen ay yüklediğim bütçe raporu".
        </p>
        <div className="flex gap-2">
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && searchPastDocuments()}
            placeholder="Ne aramıştın?"
            className="flex-1 bg-white/10 text-white rounded px-3 py-2 text-sm outline-none"
          />
          <button onClick={searchPastDocuments} disabled={searching} className="bg-white/10 rounded px-3 py-2 text-sm flex items-center gap-1 disabled:opacity-40 cursor-pointer">
            <Search size={14} /> {searching ? 'Aranıyor…' : 'Ara'}
          </button>
        </div>
        {searchResults.length > 0 && (
          <div className="mt-3 space-y-2">
            {searchResults.map((h, i) => (
              <div key={i} className="bg-black/20 rounded p-2 text-xs">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-white/60 truncate">{h.path}</p>
                  <button
                    onClick={() => { setFilePath(h.path); setFileType('document'); }}
                    className="shrink-0 text-white/40 hover:text-white underline cursor-pointer"
                  >
                    Bu dosyayı seç
                  </button>
                </div>
                <p className="text-white/40 mt-1">{h.snippet}…</p>
                <p className="text-white/20 mt-1">benzerlik: {(h.score * 100).toFixed(0)}%</p>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center gap-2">
        <button onClick={pickFile} className="bg-[#e95420] rounded px-4 py-2 text-sm cursor-pointer">Dosya Seç</button>
        {filePath && (
          <span className="text-sm text-white/50 flex items-center gap-1 truncate">
            {fileType === 'image' ? <ImageIcon size={14} /> : <FileText size={14} />}
            {filePath.split('/').pop()}
          </span>
        )}
        {filePath && fileType === 'document' && (
          <button onClick={indexCurrentFile} disabled={indexing} className="text-xs bg-white/10 rounded px-2 py-1.5 disabled:opacity-40 whitespace-nowrap cursor-pointer">
            {indexing ? 'İndeksleniyor…' : 'Hafızaya İndeksle'}
          </button>
        )}
      </div>

      {fileType === 'image' && filePath && (
        <img src={convertFileSrc(filePath)} alt="seçilen görsel" className="max-h-64 rounded-lg" />
      )}

      {models.length > 0 && (
        <div className="text-sm">
          <label className="text-white/50 mr-2">Model:</label>
          <select value={selectedModel} onChange={(e) => setSelectedModel(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1">
            {models.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
          {fileType === 'image' && (
            <span className="text-xs text-white/30 ml-2">Görsel analizi için çok-modlu (ör. llava) bir model gerekir.</span>
          )}
        </div>
      )}

      <div className="flex gap-2">
        <input
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && runAnalysis()}
          placeholder={fileType === 'image' ? 'Bu görselde ne var? Analiz et...' : 'Bu belgeyi özetle, verileri analiz et...'}
          className="flex-1 bg-white/10 text-white rounded px-3 py-2 text-sm outline-none"
        />
        <button onClick={runAnalysis} disabled={!filePath || !question.trim() || busy} className="bg-[#e95420] rounded px-4 py-2 text-sm disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer">
          {busy ? 'Analiz ediliyor…' : 'Analiz Et'}
        </button>
      </div>

      {error && <p className="text-red-400 text-sm bg-red-950/30 rounded p-3">{error}</p>}

      {result && (
        <div className="bg-white/5 rounded-lg p-4">
          <MessageContent text={result} />

          <div className="flex items-center gap-2 mt-4 pt-3 border-t border-white/10">
            <select value={exportFormat} onChange={(e) => setExportFormat(e.target.value)} className="bg-white/10 text-white rounded px-2 py-1 text-sm">
              {EXPORT_FORMATS.map((f) => <option key={f.id} value={f.id}>{f.label}</option>)}
            </select>
            <button onClick={exportResult} disabled={exporting} className="bg-[#e95420] rounded px-3 py-1.5 text-sm flex items-center gap-1 disabled:opacity-40 cursor-pointer">
              <Download size={14} /> {exporting ? 'Kaydediliyor…' : 'Dışa Aktar'}
            </button>
            {exportedPath && <span className="text-xs text-green-400">Kaydedildi: {exportedPath}</span>}
          </div>
        </div>
      )}

      <div className="text-xs text-white/40 bg-white/5 rounded p-3">
        Gerekli kurulumlar: <code className="bg-black/30 px-1 rounded">poppler-utils</code> (PDF okuma) ·{' '}
        <code className="bg-black/30 px-1 rounded">pandoc</code> (Word okuma + .docx/.pdf üretimi) ·{' '}
        <code className="bg-black/30 px-1 rounded">libreoffice</code> (.pdf üretimi). Excel okuma/yazma harici araç
        gerektirmez. Görsel analiz için Modeller sekmesinden "llava" gibi çok-modlu bir model indirmen gerekir.
        Belge arama (RAG) için "nomic-embed-text" modelini indirmen gerekir (Modeller sekmesinden).
      </div>
    </div>
  );
}
