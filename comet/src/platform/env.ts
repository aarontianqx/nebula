/** 是否运行在 Tauri 容器内（浏览器直开 Vite 页面时为 false，平台调用全部空操作降级）。 */
export const hasTauri = "__TAURI_INTERNALS__" in window;
