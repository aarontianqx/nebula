import { languages } from "./locales.js";

export { languages };

// Browser preferences are independent of the service's provider configuration.
export const settings = Object.freeze({
  defaultLanguage: "zh",
  storageKey: "transit.language",
});
const supported = (value) => Object.hasOwn(languages, value);
function initialLanguage() {
  try {
    const saved = localStorage.getItem(settings.storageKey);
    if (supported(saved)) return saved;
  } catch {
    // Private browsing or storage policy must not block the dashboard.
  }
  for (const candidate of navigator.languages || [navigator.language]) {
    const code = candidate?.split("-")[0].toLowerCase();
    if (supported(code)) return code;
  }
  return settings.defaultLanguage;
}
let currentLanguage = initialLanguage();
export const language = () => currentLanguage;
export const locale = () => languages[currentLanguage].locale;
export function t(key, values = {}) {
  const message =
    languages[currentLanguage].messages[key] ??
    languages[settings.defaultLanguage].messages[key];
  if (message == null) throw new Error(`Missing UI message: ${key}`);
  return message.replace(/\{(\w+)\}/g, (_, name) => {
    if (!Object.hasOwn(values, name))
      throw new Error(`Missing UI value: ${key}.${name}`);
    return String(values[name]);
  });
}
export function setLanguage(code, persist = true) {
  if (!supported(code)) return false;
  const changed = currentLanguage !== code;
  currentLanguage = code;
  if (persist) {
    try {
      localStorage.setItem(settings.storageKey, code);
    } catch {
      // Retain this tab's choice when persistent storage is unavailable.
    }
  }
  if (changed) window.dispatchEvent(new Event("transit:languagechange"));
  return changed;
}
export function applyTranslations(root = document) {
  document.documentElement.lang = locale();
  for (const node of root.querySelectorAll("[data-i18n]"))
    node.textContent = t(node.dataset.i18n);
  for (const attribute of ["aria-label", "placeholder", "title"]) {
    for (const node of root.querySelectorAll(`[data-i18n-${attribute}]`))
      node.setAttribute(
        attribute,
        t(node.getAttribute(`data-i18n-${attribute}`)),
      );
  }
}
window.addEventListener("storage", (event) => {
  if (event.key === settings.storageKey && supported(event.newValue))
    setLanguage(event.newValue, false);
});
