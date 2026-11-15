import { Dialog, Label, Row, Button, Column } from "./controls.mjs";
import { Message } from "./messages.mjs";

/** A real modal native subtree with a cancellable result and focus restoration. */
export class DialogController {
  static controlFactory = Object.freeze({
    topics: ["windows", "workflow", "product"],
    constructor: "new DialogController({window})",
    methods:
      "show({title,content,choices:[{id,label,accent?}],cancelValue:null,signal}), close(value), destroy(). Promise resolves the chosen id; close/owner destruction cancels and restores focus.",
  });
  constructor({ window } = {}) {
    if (!window?.root || !window.application)
      throw TypeError("Dialog controller requires a live SDK window");
    this.window = window;
  }
  show({
    title = "Dialog",
    content = [],
    choices = [{ id: "ok", label: "OK" }],
    cancelValue = null,
    signal,
  } = {}) {
    if (this.active) throw Error("A dialog is already open");
    if (this.window.presentationState.closed || signal?.aborted)
      return Promise.resolve(cancelValue);
    if (
      !Array.isArray(content) ||
      !Array.isArray(choices) ||
      !choices.length ||
      choices.length > 8 ||
      new Set(choices.map((c) => c.id)).size !== choices.length
    )
      throw TypeError("Invalid dialog choices");
    const owner = this.window,
      focus = [...owner.application.controls.values()].find(
        (control) =>
          owner.application.controlWindows.get(control.id) === owner &&
          control.getProperty("focused"),
      );
    let resolve;
    const promise = new Promise((yes) => {
      resolve = yes;
    });
    const buttons = choices.map((choice) => {
      const button = new Button({
        text: choice.label,
        accent: choice.accent === true,
      });
      button.on(Message.ACTIVATE, () => this.close(choice.id));
      return button;
    });
    const dialog = new Dialog({ text: title }, [
      ...content,
      new Row({ text: "panel:actions" }, buttons),
    ]);
    this.active = { dialog, resolve, cancelValue, focus, signal };
    const abort = () => this.close(cancelValue);
    this.active.abort = abort;
    signal?.addEventListener("abort", abort, { once: true });
    const bind = (control) => {
      control.on(Message.KEY, (event) => {
        if (event.detail?.pressed && [9].includes(event.detail.keyCode)) {
          event.preventDefault();
          this.close(cancelValue);
        }
      });
      for (const child of control.children) bind(child);
    };
    bind(dialog);
    dialog.on(Message.DESTROY, () => {
      if (this.active?.dialog === dialog) this.close(cancelValue);
    });
    owner.root.append(dialog);
    buttons[0].setProperty("focused", true);
    return promise;
  }
  close(value) {
    const active = this.active;
    if (!active) return;
    this.active = null;
    active.signal?.removeEventListener("abort", active.abort);
    if (!active.dialog.destroyed) active.dialog.destroy();
    if (active.focus && !active.focus.destroyed)
      active.focus.setProperty("focused", true);
    active.resolve(value);
  }
  choose({
    title = "Unsaved changes",
    text = "Save your changes?",
    signal,
  } = {}) {
    return this.show({
      title,
      content: [new Label({ text })],
      choices: [
        { id: "cancel", label: "Cancel" },
        { id: "discard", label: "Discard" },
        { id: "save", label: "Save", accent: true },
      ],
      cancelValue: "cancel",
      signal,
    });
  }
  destroy() {
    this.close(this.active?.cancelValue);
  }
}
