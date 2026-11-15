import { CommandHistory } from "./command-history.mjs";
import { FileStore } from "./file-store.mjs";
import { TaskController } from "./tasks.mjs";
import { Message } from "./messages.mjs";

/** Format-neutral document lifecycle, shared by any number of window views. */
export class DocumentSession {
  static controlFactory = Object.freeze({
    topics: ["documents", "workflow", "product"],
    constructor:
      'new DocumentSession({id,services,history:new CommandHistory(...),codec:{id,encode:async(value)=>Uint8Array,decode:async(bytes)=>value},chooseUnsaved:async()=>"save"|"discard"|"cancel",chooseSave:async()=>file|null,recoveryPath?})',
    methods:
      'open(file), save({file?}), canClose(), requestClose(), startAutosave({delayMs?,mode:"save"|"recovery",onError?}), checkpoint(), recover(), subscribe(listener), close(). Async operations serialize. Dirty edits made during saving stay dirty; conflicts and failed durable writes never mark saved. Recovery journals use app-relative scoped storage. Shared history drives every window view.',
  });
  constructor({
    id,
    services,
    history = new CommandHistory(),
    codec,
    store = new FileStore({ services }),
    chooseUnsaved = async () => "cancel",
    chooseSave = async () => null,
    recoveryPath = null,
  } = {}) {
    if (
      typeof id !== "string" ||
      !/^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(id) ||
      !(history instanceof CommandHistory) ||
      !codec?.id ||
      typeof codec.encode !== "function" ||
      typeof codec.decode !== "function" ||
      typeof chooseUnsaved !== "function" ||
      typeof chooseSave !== "function"
    )
      throw TypeError("Invalid document session");
    Object.assign(this, {
      id,
      services,
      history,
      codec,
      store,
      chooseUnsaved,
      chooseSave,
      recoveryPath,
    });
    this.file = null;
    this.version = undefined;
    this.closed = false;
    this.listeners = new Set();
    this.task = new TaskController({ mode: "queue" });
    this.stopHistory = history.subscribe(() => this.changed());
    this.stopTask = this.task.subscribe(() => this.changed());
  }
  get snapshot() {
    return {
      id: this.id,
      file: this.file ? { ...this.file } : null,
      dirty: this.history.dirty,
      revision: this.history.revision,
      busy: this.task.state.status === "running",
      closed: this.closed,
    };
  }
  subscribe(listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  changed() {
    for (const listener of [...this.listeners]) listener(this.snapshot);
  }
  active(signal) {
    signal?.throwIfAborted();
    if (this.closed) throw Error("Document is closed");
  }
  async permit(signal) {
    if (!this.history.dirty) return true;
    const revision = this.history.revision,
      choice = await this.chooseUnsaved(this.snapshot);
    this.active(signal);
    if (this.history.revision !== revision)
      throw Error("Document changed while deciding");
    if (choice === "cancel") return false;
    if (choice === "discard") return true;
    if (choice !== "save") throw TypeError("Invalid unsaved-changes choice");
    const saved = await this.saveNow({}, signal);
    return Boolean(saved) && !this.history.dirty;
  }
  open(file) {
    return this.task.run(async ({ signal }) => {
      this.active(signal);
      if (!(await this.permit(signal))) return false;
      const revision = this.history.revision,
        version = await this.store.version(file);
      let adopted = false;
      try {
        const bytes = await this.store.read(
            { ...file, version: version.version },
            { signal },
          ),
          value = await this.codec.decode(bytes);
        this.active(signal);
        if (revision !== this.history.revision)
          throw Error("Document changed while opening");
        this.history.replace(value);
        const previous = this.version;
        this.file = { ...file };
        this.version = version.version;
        adopted = true;
        if (previous) await this.store.releaseVersion(previous);
        this.changed();
        return true;
      } finally {
        if (!adopted) await this.store.releaseVersion(version.version);
      }
    });
  }
  save(options = {}) {
    return this.task.run(({ signal }) => this.saveNow(options, signal));
  }
  async saveNow({ file = this.file } = {}, signal) {
    this.active(signal);
    if (!file) file = await this.chooseSave(this.snapshot);
    this.active(signal);
    if (!file) return false;
    const revision = this.history.revision,
      value = this.history.value,
      sameFile =
        this.file && JSON.stringify(file) === JSON.stringify(this.file),
      data = await this.codec.encode(value);
    this.active(signal);
    const result = await this.store.write(file, data, {
      expectedVersion: sameFile ? this.version : undefined,
      signal,
      requireDurable: true,
    });
    this.active(signal);
    if (result.durable !== true)
      throw Error("Storage did not acknowledge a durable save");
    const previous = this.version;
    this.file = { ...file };
    this.version = result.version;
    if (previous && previous !== this.version)
      await this.store.releaseVersion(previous);
    if (this.history.revision === revision) this.history.markSaved();
    this.changed();
    return {
      savedRevision: revision,
      current: this.history.revision === revision,
      file: { ...this.file },
    };
  }
  checkpoint() {
    return this.task.run(async ({ signal }) => {
      this.active(signal);
      if (!this.recoveryPath) throw Error("Recovery path is not configured");
      const value = this.history.value,
        revision = this.history.revision,
        journal = {
          schema: "ricewm.document-recovery/v1",
          id: this.id,
          codec: this.codec.id,
          revision,
          value,
        };
      const data = new TextEncoder().encode(JSON.stringify(journal));
      const result = await this.store.write({ path: this.recoveryPath }, data, {
        signal,
      });
      if (result.version) await this.store.releaseVersion(result.version);
      return { revision, current: this.history.revision === revision };
    });
  }
  recover() {
    return this.task.run(async ({ signal }) => {
      this.active(signal);
      if (!this.recoveryPath) throw Error("Recovery path is not configured");
      if (!(await this.permit(signal))) return false;
      const revision = this.history.revision,
        data = await this.store.read({ path: this.recoveryPath }, { signal }),
        journal = JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(data),
        );
      if (
        journal?.schema !== "ricewm.document-recovery/v1" ||
        journal.id !== this.id ||
        journal.codec !== this.codec.id
      )
        throw TypeError("Recovery journal does not match this document");
      this.active(signal);
      if (revision !== this.history.revision)
        throw Error("Document changed during recovery");
      this.history.replace(journal.value, { saved: false });
      if (this.version) await this.store.releaseVersion(this.version);
      this.version = undefined;
      this.file = null;
      this.changed();
      return true;
    });
  }
  canClose() {
    return this.task.run(async ({ signal }) => {
      this.active(signal);
      return this.permit(signal);
    });
  }
  async requestClose() {
    const allowed = await this.canClose();
    if (allowed) this.close();
    return allowed;
  }
  startAutosave({
    delayMs = 1000,
    mode = "recovery",
    onError = () => {},
  } = {}) {
    if (
      !Number.isInteger(delayMs) ||
      delayMs < 100 ||
      delayMs > 60000 ||
      !["save", "recovery"].includes(mode) ||
      typeof onError !== "function" ||
      (mode === "recovery" && !this.recoveryPath)
    )
      throw TypeError(
        "Choose bounded autosave timing and a configured recovery path",
      );
    this.stopAutosave?.();
    let timer = null;
    const changed = () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      if (!this.history.dirty || this.closed) return;
      timer = setTimeout(() => {
        timer = null;
        const pending =
          mode === "save" && this.file
            ? this.save()
            : this.recoveryPath
              ? this.checkpoint()
              : Promise.reject(Error("Unsaved documents need a recovery path"));
        pending.catch(onError);
      }, delayMs);
    };
    const stop = this.history.subscribe(changed);
    this.stopAutosave = () => {
      stop();
      if (timer !== null) clearTimeout(timer);
      timer = null;
    };
    changed();
    return this.stopAutosave;
  }
  close() {
    if (this.closed) return;
    this.closed = true;
    this.stopAutosave?.();
    this.stopHistory();
    this.stopTask();
    this.task.close();
    if (this.version)
      void this.store.releaseVersion(this.version).catch(() => {});
    this.version = undefined;
    this.changed();
    this.listeners.clear();
  }
}

export class DocumentManager {
  static controlFactory = Object.freeze({
    topics: ["documents", "windows", "workflow"],
    constructor: "new DocumentManager()",
    methods:
      "add(session), get(id), attach(id,window), requestClose(id), close(); each shared session can own multiple window views. Closing one view keeps the document available to others.",
  });
  constructor() {
    this.documents = new Map();
    this.views = new Map();
    this.appBindings = new Map();
  }
  add(session) {
    if (!(session instanceof DocumentSession) || this.documents.has(session.id))
      throw TypeError("Document ids must be unique");
    this.documents.set(session.id, session);
    this.views.set(session.id, new Set());
    return session;
  }
  get(id) {
    const session = this.documents.get(id);
    if (!session) throw Error("Unknown document");
    return session;
  }
  attach(id, window) {
    this.get(id);
    if (!window?.onClose) throw TypeError("Document views need an SDK window");
    const views = this.views.get(id);
    views.add(window);
    const app = window.application;
    if (!this.appBindings.has(app))
      this.appBindings.set(
        app,
        app.on(Message.APP_CLOSE_REQUEST, async (detail) => {
          const selected = [];
          for (const [document, all] of this.views) {
            const own = [...all].filter(
                (view) =>
                  view.application === app && !view.presentationState.closed,
              ),
              closing = own.filter(
                (view) =>
                  detail?.window == null ||
                  String(view.id) === String(detail.window),
              );
            if (closing.length && closing.length === all.size) {
              const session = this.get(document);
              if (!session.closed) selected.push(session);
            }
          }
          const revisions = new Map();
          for (const session of selected) {
            if (!(await session.canClose())) return false;
            revisions.set(session, session.history.revision);
          }
          if (
            selected.some(
              (session) => session.history.revision !== revisions.get(session),
            )
          )
            return false;
          for (const session of selected) session.close();
          return true;
        }),
      );
    const stop = window.onClose(() => views.delete(window));
    return () => {
      views.delete(window);
      stop();
    };
  }
  async requestClose(id) {
    const session = this.get(id);
    if (!(await session.requestClose())) return false;
    for (const window of this.views.get(id)) window.requestClose();
    this.views.delete(id);
    this.documents.delete(id);
    return true;
  }
  close() {
    for (const session of this.documents.values()) session.close();
    for (const stop of this.appBindings.values()) stop();
    this.appBindings.clear();
    this.documents.clear();
    this.views.clear();
  }
}
