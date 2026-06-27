import { useEffect, useState } from "react";
import { Check, Code2, Download, Eye, ImageDown } from "lucide-react";
import { copyImage, downloadBlob, svgToPngBlob } from "../../lib/image";
import { CopyButton } from "./CopyButton";

interface Props {
  output: string;
  error: string | null;
  placeholder?: string;
}

const SIZES = [256, 512, 1024] as const;
type Size = (typeof SIZES)[number];

/**
 * 二维码结果：SVG 即时渲染，并提供「复制图片 / 下载 PNG」（可选分辨率），
 * 方便直接粘进 IM。非 SVG（ASCII）输出退化为纯文本。
 */
export function QrResult({ output, error, placeholder }: Props) {
  const trimmed = output.trimStart();
  const isSvg = !error && (trimmed.startsWith("<svg") || trimmed.startsWith("<?xml"));
  const [mode, setMode] = useState<"render" | "source">("render");
  const [size, setSize] = useState<Size>(512);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    setMode("render");
    setCopied(false);
    setHint(null);
  }, [output]);

  const handleCopyImage = async () => {
    if (!isSvg || busy) return;
    setBusy(true);
    setHint(null);
    try {
      const ok = await copyImage(output, size);
      if (ok) {
        setCopied(true);
        setTimeout(() => setCopied(false), 1400);
      } else {
        // 剪贴板不支持图片时，退化为下载。
        const blob = await svgToPngBlob(output, size);
        downloadBlob(blob, `qrcode-${size}.png`);
        setHint("剪贴板不支持图片，已下载 PNG");
      }
    } catch {
      setHint("生成图片失败");
    } finally {
      setBusy(false);
    }
  };

  const download = async () => {
    if (!isSvg || busy) return;
    setBusy(true);
    setHint(null);
    try {
      const blob = await svgToPngBlob(output, size);
      downloadBlob(blob, `qrcode-${size}.png`);
    } catch {
      setHint("生成图片失败");
    } finally {
      setBusy(false);
    }
  };

  if (error) {
    return (
      <div className="flex h-full flex-col bg-[var(--color-bg)]">
        <div className="px-4 py-2 text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
          输出
        </div>
        <div className="px-4 pb-4">
          <div className="flex items-start gap-2 rounded-lg border border-[var(--color-danger)]/40 bg-[var(--color-danger-soft)] px-3 py-2.5">
            <pre className="font-mono whitespace-pre-wrap text-sm leading-relaxed text-[var(--color-danger)]">
              {error}
            </pre>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[var(--color-bg)]">
      {/* 顶栏：标题 + 渲染/源码 + 复制源码 */}
      <div className="flex items-center justify-between px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
            输出
          </span>
          {isSvg && (
            <div className="flex items-center rounded border border-[var(--color-border)] text-xs">
              <ToggleBtn active={mode === "render"} onClick={() => setMode("render")} left>
                <Eye size={12} /> 渲染
              </ToggleBtn>
              <ToggleBtn active={mode === "source"} onClick={() => setMode("source")}>
                <Code2 size={12} /> 源码
              </ToggleBtn>
            </div>
          )}
        </div>
        {output && <CopyButton text={output} label="复制 SVG" />}
      </div>

      <div className="flex-1 overflow-auto px-4 pb-4">
        {!output ? (
          <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-faint)]">
            {placeholder ?? "结果会显示在这里"}
          </div>
        ) : !isSvg ? (
          <pre className="font-mono whitespace-pre text-xs leading-tight text-[var(--color-text)]">
            {output}
          </pre>
        ) : mode === "source" ? (
          <pre className="font-mono whitespace-pre-wrap text-sm leading-relaxed text-[var(--color-text)]">
            {output}
          </pre>
        ) : (
          <div className="animate-fade-in flex flex-col items-center gap-4">
            <div
              className="inline-block rounded-lg bg-white p-4 shadow-sm [&>svg]:block [&>svg]:h-auto [&>svg]:w-full [&>svg]:max-w-[240px]"
              // SVG 由本地 qrcode crate 生成，来源可信。
              dangerouslySetInnerHTML={{ __html: output }}
            />

            {/* 图片导出工具条 */}
            <div className="flex flex-col items-center gap-2">
              <div className="flex items-center gap-2">
                <span className="text-xs text-[var(--color-text-faint)]">分辨率</span>
                <div className="flex items-center rounded-md border border-[var(--color-border)] p-0.5">
                  {SIZES.map((s) => (
                    <button
                      key={s}
                      onClick={() => setSize(s)}
                      className={[
                        "rounded px-2 py-0.5 text-xs transition-colors",
                        size === s
                          ? "bg-[var(--color-accent)] text-white"
                          : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]",
                      ].join(" ")}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={handleCopyImage}
                  disabled={busy}
                  className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
                >
                  {copied ? <Check size={14} /> : <ImageDown size={14} />}
                  {copied ? "已复制图片" : "复制图片"}
                </button>
                <button
                  onClick={download}
                  disabled={busy}
                  className="flex items-center gap-1.5 rounded-md border border-[var(--color-border-strong)] px-3 py-1.5 text-sm text-[var(--color-text)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
                >
                  <Download size={14} />
                  下载 PNG
                </button>
              </div>
              {hint && <span className="text-xs text-[var(--color-warning)]">{hint}</span>}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function ToggleBtn({
  active,
  onClick,
  left = false,
  children,
}: {
  active: boolean;
  onClick: () => void;
  left?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={[
        "flex items-center gap-1 px-2 py-0.5 transition-colors",
        left ? "rounded-l" : "rounded-r",
        active
          ? "bg-[var(--color-accent)] text-white"
          : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]",
      ].join(" ")}
    >
      {children}
    </button>
  );
}
