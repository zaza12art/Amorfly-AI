import { useState } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { Check, Copy } from 'lucide-react';
import { copyToClipboard } from '../lib/clipboard';

interface CodeBlockProps {
  code: string;
  language?: string;
  /** Sağ üstte "Dosya Olarak Kaydet" gibi ek bir aksiyon göstermek istersen. */
  extraAction?: React.ReactNode;
}

/** Amorfly genelinde (Sohbet, Belge Analizi, Dil Eğitimi) kullanılan tek
 * kod bloğu bileşeni — gerçek syntax highlighting (react-syntax-highlighter,
 * Prism motoru, VS Code Dark+ teması) + kopyalama butonu.
 *
 * "Prism" (Light değil) tam sürümünü kullanıyoruz: ~180 dili built-in
 * destekliyor, tek tek dil kaydı (registerLanguage) gerekmiyor — AI'ın
 * ürettiği kod bloğu hangi dilde olursa olsun (nadir bir dil bile olsa)
 * çalışır. Masaüstü uygulaması olduğumuz için ekstra birkaç yüz KB bundle
 * boyutu (web sayfası gibi) kritik bir sorun değil.
 */
export default function CodeBlock({ code, language, extraAction }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    const ok = await copyToClipboard(code);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  }

  return (
    <div className="bg-[#1e1e1e] rounded-lg my-1 overflow-hidden border border-white/5">
      <div className="flex items-center justify-between px-3 py-1.5 bg-black/30 text-xs text-white/50">
        <span>{language || 'metin'}</span>
        <div className="flex items-center gap-3">
          <button onClick={handleCopy} className="flex items-center gap-1 hover:text-white cursor-pointer">
            {copied ? <Check size={13} /> : <Copy size={13} />}
            {copied ? 'Kopyalandı' : 'Kopyala'}
          </button>
          {extraAction}
        </div>
      </div>
      <SyntaxHighlighter
        language={language || 'text'}
        style={vscDarkPlus}
        customStyle={{ margin: 0, padding: '12px', fontSize: '12px', background: 'transparent' }}
        wrapLongLines
      >
        {code}
      </SyntaxHighlighter>
    </div>
  );
}
