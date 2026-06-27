/** 文本度量辅助：用于输入/输出区的「行 · 字符」徽标。 */

/** Unicode 友好的字符数（按 code point，emoji 计 1）。 */
export function charCount(text: string): number {
  return Array.from(text).length;
}

export function lineCount(text: string): number {
  if (text.length === 0) return 0;
  return text.split("\n").length;
}

/**
 * 紧凑的度量标签：单行只显示字符数，多行显示「N 行 · M 字符」。
 * 空文本返回 null（不渲染徽标）。
 */
export function countLabel(text: string): string | null {
  if (text.length === 0) return null;
  const chars = charCount(text);
  const lines = lineCount(text);
  return lines > 1 ? `${lines} 行 · ${chars} 字符` : `${chars} 字符`;
}
