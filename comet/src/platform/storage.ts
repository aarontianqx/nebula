/**
 * localStorage 读写助手：统一"非法值回退默认"的解析逻辑，
 * 各功能模块只声明自己的 key 与默认值。
 */

/** 读取分钟数配置（≥1 的有限数），非法回退 fallback。 */
export function readMinutes(key: string, fallback: number): number {
  const raw = Number(localStorage.getItem(key));
  return Number.isFinite(raw) && raw >= 1 ? raw : fallback;
}

/** 读取 epoch 毫秒时间戳，缺失/非法返回 0。 */
export function readTimestamp(key: string): number {
  const raw = Number(localStorage.getItem(key));
  return Number.isFinite(raw) && raw > 0 ? raw : 0;
}

/** 读取 JSON 值，缺失或解析失败返回 null。 */
export function readJSON<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

/** 写入 JSON 值；传 null 删除该 key。 */
export function writeJSON(key: string, value: unknown | null): void {
  if (value === null) localStorage.removeItem(key);
  else localStorage.setItem(key, JSON.stringify(value));
}
