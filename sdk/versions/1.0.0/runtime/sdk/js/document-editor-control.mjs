import { TextEditor } from "./controls.mjs";
import { Message } from "./messages.mjs";
import { TaskController } from "./tasks.mjs";
const bytes = (text) => new TextEncoder().encode(text).length;
/** A retained native paged editor. The shared engine owns rich document data. */
export class DocumentEditor extends TextEditor {
  static family = "composite";
  static constructorOptions = Object.freeze(["engine", "onError"]);
  static authoring = Object.freeze({
    constructor:
      "new DocumentEditor({engine:await openDocumentEngine(...),onError?,name})",
    methods:
      'refresh(), setScroll(y), format("inline"|"block"|"link",data), undo(), redo(); ready resolves the first real native paged view. Native input edits only the currently projected UTF-8 window; the source-backed engine preserves the rest of the document. Declare licensed font resources. Engine state, commands and serialization are reusable by other views.',
  });
  constructor({ engine, onError = () => {}, ...properties } = {}) {
    if (
      !engine?.presentation ||
      !engine?.command ||
      typeof onError !== "function"
    )
      throw TypeError("DocumentEditor needs a live document engine");
    super(properties);
    Object.assign(this, { engine, onError });
    this.scroll = 0;
    this.view = null;
    this.task = new TaskController({ mode: "queue" });
    this.localChange = false;
    this.on(Message.VALUE_CHANGED, () => {
      if (this.localChange || !this.view) return;
      const old = this.view,
        next = this.getProperty("value"),
        selection = this.selection;
      return this.work(async () => {
        const before = Array.from(old.text),
          after = Array.from(next);
        let prefix = 0,
          suffix = 0;
        while (
          prefix < before.length &&
          prefix < after.length &&
          before[prefix] === after[prefix]
        )
          prefix++;
        while (
          suffix < before.length - prefix &&
          suffix < after.length - prefix &&
          before[before.length - 1 - suffix] ===
            after[after.length - 1 - suffix]
        )
          suffix++;
        await engine.command(
          "replace",
          {
            start: old.start + bytes(before.slice(0, prefix).join("")),
            end:
              old.start +
              bytes(before.slice(0, before.length - suffix).join("")),
            text: after.slice(prefix, after.length - suffix).join(""),
          },
          { expectedRevision: old.revision },
        );
        await this.refreshNow();
      });
    });
    this.stopEngine = engine.subscribe(() => {
      if (!this.localChange && this.task.state.status !== "running")
        void engine
          .whenIdle()
          .then(() => (this.destroyed ? null : this.refresh()))
          .catch(onError);
    });
    this.ready = this.refresh();
    this.ready.catch(onError);
  }
  async work(work) {
    return this.task.run(async () => {
      const enabled = this.enabled;
      this.enabled = false;
      this.localChange = true;
      try {
        return await work();
      } catch (error) {
        this.onError(error);
        await this.refreshNow().catch(() => {});
        throw error;
      } finally {
        this.localChange = false;
        this.enabled = enabled;
      }
    });
  }
  async refreshNow() {
    const view = await this.engine.presentation({ scroll: this.scroll });
    if (
      !view ||
      typeof view.text !== "string" ||
      typeof view.language !== "string" ||
      !view.language.startsWith("rich~o") ||
      !Number.isInteger(view.start) ||
      !Number.isInteger(view.end) ||
      view.start > view.end ||
      !Number.isInteger(view.revision)
    )
      throw TypeError("Invalid native document presentation");
    this.view = Object.freeze({ ...view });
    this.setProperty("value", view.text, { notify: false });
    this.setSelection(view.anchor, view.caret);
    this.application?.invalidate(this);
    return view;
  }
  refresh() {
    return this.work(() => this.refreshNow());
  }
  setScroll(y) {
    if (!Number.isInteger(y) || y < 0 || y > 0xffffffff)
      throw RangeError("Invalid document scroll");
    this.scroll = y;
    return this.refresh();
  }
  async syncSelection() {
    const selection = this.selection,
      chars = Array.from(this.getProperty("value"));
    await this.engine.command(
      "select",
      {
        anchor:
          this.view.start + bytes(chars.slice(0, selection.anchor).join("")),
        caret:
          this.view.start + bytes(chars.slice(0, selection.caret).join("")),
      },
      { expectedRevision: this.view.revision },
    );
  }
  format(operation, data) {
    if (!["inline", "block", "link"].includes(operation))
      return Promise.reject(TypeError("Invalid document formatting operation"));
    return this.work(async () => {
      await this.syncSelection();
      await this.engine.command(operation, data);
      return this.refreshNow();
    });
  }
  undo() {
    return this.work(async () => {
      await this.engine.command("undo");
      return this.refreshNow();
    });
  }
  redo() {
    return this.work(async () => {
      await this.engine.command("redo");
      return this.refreshNow();
    });
  }
  toWidget() {
    const widget = super.toWidget();
    if (!this.view) return widget;
    const text = this.editorController()
      .presentation()
      .replace("editor:plain:", `editor:${this.view.language}:`);
    if (new TextEncoder().encode(text).length > 16384)
      throw RangeError("Document presentation exceeds native budget");
    return Object.freeze({ ...widget, text });
  }
  destroy() {
    this.stopEngine?.();
    this.task.close();
    return super.destroy();
  }
}
