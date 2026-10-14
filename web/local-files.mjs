// Browser-local storage adapter for the original RiceWM VFS protocol.
import { RiceVfsProvider } from "./rice-vfs-provider.mjs";
const fail = (code, message) => {
  throw Object.assign(Error(message), { code });
};
export async function openLocalFiles() {
  const database = await new Promise((yes, no) => {
    const r = indexedDB.open("ricewm-public-files", 1);
    r.onupgradeneeded = () => r.result.createObjectStore("entries");
    r.onsuccess = () => yes(r.result);
    r.onerror = () => no(r.error);
  });
  const records = new Map(
    await new Promise((yes, no) => {
      const tx = database.transaction("entries"),
        s = tx.objectStore("entries"),
        keys = s.getAllKeys(),
        values = s.getAll();
      tx.oncomplete = () =>
        yes(keys.result.map((key, i) => [key, values.result[i]]));
      tx.onerror = () => no(tx.error);
    }),
  );
  const persist = async () =>
    await new Promise((yes, no) => {
      const tx = database.transaction("entries", "readwrite"),
        s = tx.objectStore("entries");
      s.clear();
      for (const [path, entry] of records) s.put(entry, path);
      tx.oncomplete = yes;
      tx.onerror = () => no(tx.error);
      tx.onabort = () => no(tx.error || Error("Storage transaction aborted"));
    });
  const folder = () => ({
    kind: "directory",
    data: new Uint8Array(),
    modified: Date.now(),
    mode: 0o755,
  });
  if (!records.size) {
    for (const path of [
      "/",
      "/root",
      "/root/Desktop",
      "/root/Documents",
      "/root/Pictures",
      "/root/Downloads",
      "/root/Videos",
      "/root/.local",
      "/root/.local/share",
      "/root/.local/share/Trash",
      "/root/.local/share/Trash/files",
      "/root/.local/share/Trash/info",
    ])
      records.set(path, folder());
    await persist();
  }
  const get = (path) =>
      records.get(path) || fail("ENOENT", "No such file: " + path),
    parent = (path) => path.slice(0, path.lastIndexOf("/")) || "/";
  const metadata = (entry) => ({
    kind: entry.kind,
    size: BigInt(entry.data.length),
    modifiedMillis: BigInt(entry.modified),
    mode: entry.mode,
  });
  const change = async (work) => {
    const previous = new Map(records);
    try {
      const value = work();
      await persist();
      return value;
    } catch (error) {
      records.clear();
      for (const [p, e] of previous) records.set(p, e);
      throw error;
    }
  };
  const create = (path, entry) => {
    if (records.has(path)) fail("EEXIST", "Path already exists");
    if (get(parent(path)).kind !== "directory")
      fail("ENOTDIR", "Parent is not a folder");
    records.set(path, entry);
  };
  const callbacks = {
    stat: ({ path }) => metadata(get(path)),
    read: ({ path, offset, length }) => {
      const e = get(path);
      if (e.kind === "directory") fail("EISDIR", "Cannot read a folder");
      return e.data.slice(Number(offset), Number(offset) + length);
    },
    readDirectory: ({ path, cursor = 0n, limit }) => {
      if (get(path).kind !== "directory") fail("ENOTDIR", "Not a folder");
      const entries = [...records]
        .filter(([p]) => p !== path && parent(p) === path)
        .sort(([a], [b]) => a.localeCompare(b));
      const begin = Number(cursor);
      return {
        entries: entries
          .slice(begin, begin + limit)
          .map(([p, e]) => ({
            name: p.split("/").at(-1),
            metadata: metadata(e),
            mimeType: e.kind === "directory" ? "inode/directory" : "text/plain",
          })),
        nextCursor:
          begin + limit < entries.length ? BigInt(begin + limit) : null,
      };
    },
    write: ({ path, offset, data }) =>
      change(() => {
        const old = records.get(path);
        if (old?.kind === "directory") fail("EISDIR", "Cannot write a folder");
        if (!old) get(parent(path));
        const start = Number(offset);
        if (start + data.length > 16 * 1024 * 1024)
          fail("EINVAL", "File exceeds browser adapter limit");
        const bytes = new Uint8Array(
          Math.max(old?.data.length || 0, start + data.length),
        );
        if (old) bytes.set(old.data);
        bytes.set(data, start);
        records.set(path, {
          kind: "file",
          data: bytes,
          modified: Date.now(),
          mode: old?.mode || 0o644,
        });
        return data.length;
      }),
    mkdir: ({ path }) => change(() => create(path, folder())),
    rename: ({ from, to }) =>
      change(() => {
        get(from);
        if (records.has(to)) fail("EEXIST", "Destination exists");
        get(parent(to));
        if (to.startsWith(from + "/"))
          fail("EINVAL", "Cannot move a folder inside itself");
        const moved = [...records].filter(
          ([p]) => p === from || p.startsWith(from + "/"),
        );
        for (const [p, e] of moved) {
          records.delete(p);
          records.set(to + p.slice(from.length), e);
        }
      }),
    remove: ({ path, recursive }) =>
      change(() => {
        get(path);
        if (path === "/" || path === "/root")
          fail("EPERM", "Cannot remove the home folder");
        const descendants = [...records.keys()].filter((p) =>
          p.startsWith(path + "/"),
        );
        if (descendants.length && !recursive)
          fail("ENOTEMPTY", "Folder is not empty");
        for (const p of [...descendants, path]) records.delete(p);
      }),
    copy: ({ from, to }) =>
      change(() => {
        get(from);
        if (records.has(to)) fail("EEXIST", "Destination exists");
        get(parent(to));
        for (const [p, e] of [...records])
          if (p === from || p.startsWith(from + "/"))
            records.set(to + p.slice(from.length), {
              ...e,
              data: e.data.slice(),
            });
      }),
    setMode: ({ path, mode }) =>
      change(() => {
        records.set(path, { ...get(path), mode });
      }),
  };
  return { provider: new RiceVfsProvider(callbacks), callbacks, records };
}
