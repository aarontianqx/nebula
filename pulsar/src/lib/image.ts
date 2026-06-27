// SVG → 位图光栅化与剪贴板/下载工具。
//
// 二维码以 SVG 即时生成（矢量、清晰），但 IM 软件通常只接受位图。
// 这里在前端用 canvas 把 SVG 光栅化为指定分辨率的位图：
// - 复制到系统剪贴板：优先走 Tauri clipboard 插件（原生剪贴板，IM 可直接粘贴），
//   退化到浏览器 ClipboardItem。
// - 下载 PNG：canvas → PNG Blob。

import { api } from "./ipc";

/** 把 SVG 渲染到指定边长（像素）的离屏 canvas。正方形输出，适合二维码。 */
async function rasterize(svg: string, size: number): Promise<HTMLCanvasElement> {
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const img = await loadImage(url);
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("无法创建画布上下文");
    // 二维码是硬边像素块：关闭平滑，避免边缘发糊。
    ctx.imageSmoothingEnabled = false;
    // 白底（透明在部分 IM 里会变黑）。
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, size, size);
    ctx.drawImage(img, 0, 0, size, size);
    return canvas;
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** 把 SVG 源码渲染成指定边长的 PNG Blob。 */
export async function svgToPngBlob(svg: string, size: number): Promise<Blob> {
  const canvas = await rasterize(svg, size);
  return canvasToBlob(canvas);
}

/**
 * 把 SVG 渲染为位图并复制到系统剪贴板。返回是否成功（不支持时返回 false，调用方应退化为下载）。
 *
 * 在 Tauri 中走原生剪贴板插件（传 RGBA 原始像素，无需后端图片解码特性），最稳；
 * 纯浏览器环境退化到 `ClipboardItem`（WKWebView 可能不支持图片）。
 */
export async function copyImage(svg: string, size: number): Promise<boolean> {
  const canvas = await rasterize(svg, size);

  // 1) Tauri 原生剪贴板（首选）。
  if (api.isAvailable()) {
    try {
      const ctx = canvas.getContext("2d");
      if (!ctx) throw new Error("no ctx");
      const { data, width, height } = ctx.getImageData(0, 0, size, size);
      const { Image } = await import("@tauri-apps/api/image");
      const { writeImage } = await import("@tauri-apps/plugin-clipboard-manager");
      const image = await Image.new(new Uint8Array(data.buffer), width, height);
      await writeImage(image);
      return true;
    } catch {
      // 落到浏览器路径再试一次。
    }
  }

  // 2) 浏览器 ClipboardItem 兜底。
  try {
    if (typeof ClipboardItem === "undefined" || !navigator.clipboard?.write) {
      return false;
    }
    const blob = await canvasToBlob(canvas);
    await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
    return true;
  } catch {
    return false;
  }
}

/** 触发浏览器下载一个 Blob。 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  // 稍后回收，确保下载已触发。
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("SVG 加载失败"));
    img.src = src;
  });
}

function canvasToBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error("PNG 编码失败"));
    }, "image/png");
  });
}
