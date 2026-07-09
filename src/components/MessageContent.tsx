import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Check, Copy } from 'lucide-react';
import { copyToClipboard } from '../lib/clipboard';
import CodeBlock from './CodeBlock';

function CopyButton({ getText, className }: { getText: () => string; className?: string }) {
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
        'flex items-center gap-1 text-xs rounded px-2 py-1 bg-white/10 hover:bg-white/20 transition cursor-pointer ' +
        (className ?? '')
      }
    >
      {copied ? <Check size={12} /> : <Copy size={12} />}
      {copied ? 'Kopyalandı' : 'Kopyala'}
    </button>
  );
}

interface MessageContentProps {
  text: string;
  /** Mesajın tamamını kopyalayan buton sağ altta gösterilsin mi (varsayılan: evet) */
  showMessageCopy?: boolean;
}

/** Sohbet/analiz cevaplarında ortak kullanılan render bileşeni: markdown
 * (tablo dahil) + kod bloklarında gerçek syntax highlighting + ayrı
 * kopyalama + mesajın tamamı için sağ altta genel kopyalama butonu.
 * Hiçbir karakter/kelime sınırı yok. */
export default function MessageContent({ text, showMessageCopy = true }: MessageContentProps) {
  return (
    <div>
      <div className="text-sm [&_table]:border-collapse [&_td]:border [&_td]:border-white/20 [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-white/20 [&_th]:px-2 [&_th]:py-1 [&_th]:bg-white/10 [&_p]:my-1 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:pl-5">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={{
            // react-markdown fenced/blok kodu <pre><code> içine sarar —
            // kendi kart görünümümüzü (CodeBlock) kullanabilmek için
            // dış <pre> sarmalayıcıyı kaldırıp render'ı <code>'a bırakıyoruz.
            pre: ({ children }) => <>{children}</>,
            code({ className, children }) {
              const match = /language-(\w+)/.exec(className || '');
              const codeStr = String(children).replace(/\n$/, '');
              // Fenced blok (dil etiketli) ya da birden çok satır içeren
              // kod -> gerçek CodeBlock (syntax highlighting + kopyalama).
              // Tek satırlık, etiketsiz -> satır-içi kod (ör. `değişken`).
              if (match || codeStr.includes('\n')) {
                return <CodeBlock code={codeStr} language={match?.[1]} />;
              }
              return <code className="bg-white/10 rounded px-1">{children}</code>;
            },
          }}
        >
          {text}
        </ReactMarkdown>
      </div>
      {showMessageCopy && (
        <div className="flex justify-end mt-1">
          <CopyButton getText={() => text} />
        </div>
      )}
    </div>
  );
}
