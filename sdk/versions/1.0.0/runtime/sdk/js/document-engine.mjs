import { TaskController } from "./tasks.mjs";
const CAPACITY = 65536,
  MAX_FILE = 8 * 1024 * 1024;
const validPath = (path) =>
  typeof path === "string" &&
  /^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]{1,255}$/.test(path);
const encoder = new TextEncoder(),
  decoder = new TextDecoder("utf-8", { fatal: true });
/** The existing rich/Office model, isolated from UI and all host authority. */
export async function openDocumentEngine({
  services,
  path = "document.wasm",
  fontManifest = "document-fonts.json",
  create = { text: "", format: "docx" },
  signal,
} = {}) {
  if (
    !services?.compute ||
    !services?.assets ||
    !validPath(path) ||
    !validPath(fontManifest)
  )
    throw TypeError(
      "Document engine needs package assets and approved compute",
    );
  let handle,
    pointer,
    closed = false,
    fonts = null,
    capabilities,
    snapshot = { revision: 0 },
    loadedFonts = new Set();
  const listeners = new Set(),
    task = new TaskController();
  const alive = () => {
    if (closed) throw Error("Document engine closed");
    signal?.throwIfAborted();
  };
  const invoke = async (name, args = [], extra = {}) => {
    alive();
    try {
      return await services.compute.invoke({
        abi: 1,
        handle,
        name,
        args,
        timeoutMs: 4000,
        ...extra,
      });
    } catch (error) {
      await close().catch(() => {});
      throw error;
    }
  };
  async function close() {
    if (closed) return;
    closed = true;
    task.close();
    listeners.clear();
    signal?.removeEventListener("abort", abort);
    if (handle) await services.compute.cancel({ handle });
  }
  const abort = () => {
    void close().catch(() => {});
  };
  signal?.addEventListener("abort", abort, { once: true });
  async function raw(request) {
    const data = encoder.encode(JSON.stringify(request));
    if (data.length > CAPACITY)
      throw RangeError("Document command exceeds 64 KiB");
    const reply = await invoke("document_request", [data.length], {
      write: { offset: pointer, data },
      read: { offset: pointer, lengthFromResult: true, maxLength: CAPACITY },
    });
    if (
      !(reply.data instanceof Uint8Array) ||
      reply.data.length !== reply.value ||
      reply.value < 1
    )
      throw Error("Invalid document response");
    const response = JSON.parse(decoder.decode(reply.data));
    if (response?.ok !== true)
      throw Object.assign(
        Error(response?.error || "Document operation failed"),
        { revision: response?.revision, missingFonts: response?.missingFonts },
      );
    return response.result;
  }
  async function upload(data, { font = false } = {}) {
    if (
      !(data instanceof Uint8Array) ||
      data.length > (font ? 20 * 1024 * 1024 : MAX_FILE)
    )
      throw RangeError("Document upload exceeds 8 MiB");
    if (
      (
        await invoke(font ? "document_font_begin" : "document_input_begin", [
          data.length,
        ])
      ).value !== 0
    )
      throw Error("Document upload rejected");
    for (let offset = 0; offset < data.length; offset += CAPACITY) {
      const chunk = data.slice(offset, offset + CAPACITY);
      if (
        (
          await invoke("document_input_write", [offset, chunk.length], {
            write: { offset: pointer, data: chunk },
          })
        ).value !== 0
      )
        throw Error("Document upload is incomplete");
    }
  }
  function adopt(next) {
    if (
      !next ||
      !Number.isSafeInteger(next.revision) ||
      next.revision < snapshot.revision
    )
      throw Error("Invalid document revision");
    snapshot = Object.freeze({ ...next });
    for (const listener of [...listeners]) listener(snapshot);
    return snapshot;
  }
  async function loadFonts(mask) {
    if (!fonts) {
      const stat = await services.assets.stat({ path: fontManifest });
      if (
        !Number.isSafeInteger(stat.size) ||
        stat.size < 1 ||
        stat.size > CAPACITY
      )
        throw Error("Document font manifest is unavailable");
      const bytes = await services.assets.read({
        path: fontManifest,
        offset: 0,
        length: stat.size,
      });
      const manifest = JSON.parse(decoder.decode(bytes));
      if (
        manifest?.version !== 1 ||
        !Array.isArray(manifest.fonts) ||
        manifest.fonts.length > 20
      )
        throw TypeError("Invalid document font manifest");
      fonts = new Map();
      for (const font of manifest.fonts) {
        if (
          !Number.isInteger(font.id) ||
          font.id < 0 ||
          font.id >= 20 ||
          fonts.has(font.id) ||
          !Array.isArray(font.parts) ||
          !font.parts.length ||
          font.parts.length > 20 ||
          font.parts.some(
            (part) =>
              typeof part.resource !== "string" ||
              !Number.isInteger(part.bytes) ||
              part.bytes < 1 ||
              part.bytes > 1024 * 1024,
          )
        )
          throw TypeError("Invalid document font resource");
        fonts.set(font.id, font);
      }
    }
    for (let id = 0; id < 20; id++)
      if (mask & (1 << id)) {
        if (loadedFonts.has(id)) continue;
        const font = fonts.get(id);
        if (!font)
          throw Object.assign(
            Error("Required document font is unavailable: " + id),
            {
              code: "FONT_RESOURCE_REQUIRED",
              font: capabilities.fontResources.find((f) => f.id === id),
            },
          );
        if (!services.resources)
          throw Error("Shared SDK font resources require resources.stat/read");
        const size = font.parts.reduce((n, p) => n + p.bytes, 0);
        if (size > 20 * 1024 * 1024)
          throw RangeError("Document font exceeds its upload budget");
        const data = new Uint8Array(size);
        let destination = 0;
        for (const part of font.parts) {
          const stat = await services.resources.stat({ key: part.resource });
          if (stat.size !== part.bytes)
            throw Error("SDK font size differs from its manifest");
          for (let offset = 0; offset < part.bytes; offset += CAPACITY) {
            const length = Math.min(CAPACITY, part.bytes - offset),
              chunk = await services.resources.read({
                key: part.resource,
                offset,
                length,
              });
            if (!(chunk instanceof Uint8Array) || chunk.length !== length)
              throw Error("Incomplete document font read");
            data.set(chunk, destination + offset);
          }
          destination += part.bytes;
        }
        await upload(data, { font: true });
        await raw({ operation: "install-font", id });
        loadedFonts.add(id);
      }
  }
  async function query(operation, data) {
    try {
      return await raw({ operation, ...data });
    } catch (error) {
      if (
        !error.missingFonts ||
        !["layout", "presentation"].includes(operation)
      )
        throw error;
      await loadFonts(error.missingFonts);
      return raw({ operation, ...data });
    }
  }
  try {
    alive();
    const opened = await services.compute.open({
      abi: 1,
      path,
      profile: "rich-document",
      timeoutMs: 4000,
    });
    handle = opened.handle;
    if (
      (await invoke("document_abi_version")).value !== 1 ||
      (await invoke("document_buffer_capacity")).value !== CAPACITY
    )
      throw Error("Unsupported document ABI");
    pointer = (await invoke("document_buffer_ptr")).value;
    if (!Number.isSafeInteger(pointer) || pointer < 1)
      throw Error("Invalid document buffer");
    capabilities = await raw({ operation: "capabilities" });
    if (capabilities?.abi !== 1 || !Array.isArray(capabilities.fontResources))
      throw Error("Invalid document capabilities");
    if (create !== null)
      adopt(await raw({ operation: "create", expectedRevision: 0, ...create }));
    else adopt(await raw({ operation: "status" }));
  } catch (error) {
    await close().catch(() => {});
    throw error;
  }
  return Object.freeze({
    version: 1,
    capabilities,
    get revision() {
      return snapshot.revision;
    },
    get state() {
      return snapshot;
    },
    whenIdle: () => task.tail,
    subscribe(listener) {
      if (typeof listener !== "function")
        throw TypeError("Document listener required");
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    command(operation, data = {}, options = {}) {
      if (
        ![
          "create",
          "replace",
          "select",
          "inline",
          "block",
          "link",
          "undo",
          "redo",
        ].includes(operation) ||
        !data ||
        typeof data !== "object" ||
        Array.isArray(data) ||
        Object.keys(data).some((k) =>
          ["operation", "expectedRevision"].includes(k),
        )
      )
        return Promise.reject(TypeError("Invalid document command"));
      const expectedRevision = options.expectedRevision ?? snapshot.revision;
      return task.run(async () =>
        adopt(await raw({ operation, ...data, expectedRevision })),
      );
    },
    readText({ start = 0, length = 0 } = {}) {
      return task.run(() => query("text", { start, length }));
    },
    layout({ startLine = 0, limit = 32 } = {}) {
      return task.run(() => query("layout", { startLine, limit }));
    },
    presentation({ scroll = 0 } = {}) {
      return task.run(() => query("presentation", { scroll }));
    },
    importDocument(data, { format = "docx" } = {}) {
      const owned = data instanceof Uint8Array ? data.slice() : data,
        revision = snapshot.revision;
      return task.run(async () => {
        await upload(owned);
        return adopt(
          await raw({ operation: "open", format, expectedRevision: revision }),
        );
      });
    },
    exportDocument({ format = snapshot.format } = {}) {
      const revision = snapshot.revision;
      return task.run(async () => {
        const result = await raw({
          operation: "export",
          format,
          expectedRevision: revision,
        });
        if (
          !Number.isSafeInteger(result.size) ||
          result.size < 0 ||
          result.size > MAX_FILE ||
          result.revision !== revision
        )
          throw Error("Invalid document export");
        const bytes = new Uint8Array(result.size);
        for (let offset = 0; offset < bytes.length; offset += CAPACITY) {
          const length = Math.min(CAPACITY, bytes.length - offset),
            reply = await invoke("document_output_read", [offset, length], {
              read: { offset: pointer, length },
            });
          if (
            reply.value !== length ||
            !(reply.data instanceof Uint8Array) ||
            reply.data.length !== length
          )
            throw Error("Incomplete document export");
          bytes.set(reply.data, offset);
        }
        return bytes;
      });
    },
    close,
  });
}
openDocumentEngine.controlFactory = Object.freeze({
  topics: ["documents", "workflow", "media"],
  signature:
    'await openDocumentEngine({services,path:"document.wasm",fontManifest:"document-fonts.json",create:{text:"",format:"docx"},signal})',
  behavior:
    'Source-backed rich/Office document model: bounded DOCX/ODT imports and exports, inline/paragraph formatting, links, selection, undo/redo, paged native presentation and real font shaping. UTF-8 byte positions are authoritative. Read capabilities/fontResources. sdk.resources ["rich-document"] bundles engine and licensed Latin fonts; request required optional font resources explicitly. Missing fonts and unsupported format conversion fail without flattening the document. One engine per resource lease; close releases it.',
});
