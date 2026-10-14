import english from "./locale-en.mjs";
export const riceLocale = () => "en";
export const resolveLanguage = () => "en";
export const loadRiceLocale = async () => "en";
export const ensureLocaleFonts = async () => {};
export const setRiceLocale = () => "en";
export const t = (key, values = {}) =>
  String(english[key] ?? key).replace(
    /\{([^}]+)\}/g,
    (match, name) => values[name] ?? match,
  );
