import { validateAppCapabilities } from "./app-capabilities.mjs";
/** Installed catalog summaries are reference data; live schemas own invocation. */
export function encodeAppCapabilitySummary(descriptors = []) {
  const summaries = validateAppCapabilities(descriptors).map(
    ({ id, title, description, effect, undoable }) => ({
      id,
      title: Array.from(title.trim()).slice(0, 64).join(""),
      description: Array.from(description.trim()).slice(0, 96).join(""),
      effect,
      undoable,
    }),
  );
  const encoder = new TextEncoder();
  let data = encoder.encode(JSON.stringify(summaries));
  if (data.length > 16384) {
    for (const entry of summaries) {
      entry.description = "";
      entry.title = Array.from(entry.title).slice(0, 8).join("");
    }
    data = encoder.encode(JSON.stringify(summaries));
  }
  if (data.length > 16384)
    throw RangeError("Capability summary exceeds catalog budget");
  let binary = "";
  for (const byte of data) binary += String.fromCharCode(byte);
  return btoa(binary);
}
export function decodeAppCapabilitySummary(value) {
  if (value === undefined || value === "") return [];
  try {
    if (
      typeof value !== "string" ||
      value.length > 24000 ||
      !/^[A-Za-z0-9+/]*={0,2}$/.test(value)
    )
      return [];
    const binary = atob(value);
    if (binary.length > 16384) return [];
    const entries = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(
        Uint8Array.from(binary, (c) => c.charCodeAt(0)),
      ),
    );
    if (!Array.isArray(entries) || entries.length > 64) return [];
    const ids = new Set();
    for (const entry of entries) {
      if (
        !entry ||
        Object.keys(entry).some(
          (key) =>
            !["id", "title", "description", "effect", "undoable"].includes(key),
        ) ||
        typeof entry.id !== "string" ||
        !/^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(entry.id) ||
        ids.has(entry.id) ||
        typeof entry.title !== "string" ||
        !entry.title.trim() ||
        entry.title.length > 256 ||
        typeof entry.description !== "string" ||
        entry.description.length > 384 ||
        !["query", "mutation", "external"].includes(entry.effect) ||
        typeof entry.undoable !== "boolean"
      )
        return [];
      ids.add(entry.id);
    }
    return entries;
  } catch {
    return [];
  }
}
