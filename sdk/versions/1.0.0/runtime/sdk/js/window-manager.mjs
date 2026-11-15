import { Message } from "./messages.mjs";
import { DialogController } from "./dialog-controller.mjs";

export class WindowManager {
  static controlFactory = Object.freeze({
    topics: ["windows", "documents", "workflow"],
    constructor: "new WindowManager({app})",
    methods:
      "create({key,owner?,...createWindow options}), get(key), close(key), dialog(window), dispose(). Named app-owned windows share the app model and have independent native identities. Owner closure requests closure of its child windows. In-window modal dialogs use DialogController.",
  });
  constructor({ app } = {}) {
    if (!app?.createWindow) throw TypeError("WindowManager needs an SDK app");
    this.app = app;
    this.windows = new Map();
    this.dialogs = new Map();
    this.stops = new Map();
  }
  create({ key, owner = null, modal = false, ...options } = {}) {
    if (
      typeof key !== "string" ||
      !/^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(key) ||
      this.windows.has(key) ||
      (owner &&
        (owner.application !== this.app || owner.presentationState.closed)) ||
      typeof modal !== "boolean" ||
      (modal && !owner)
    )
      throw TypeError("Invalid named window or owner");
    if (
      owner &&
      !this.app.manifest.permissions?.services?.includes("windows.register")
    )
      throw Error("Owned windows require windows.register");
    const window = this.app.createWindow(options),
      stopOwner = owner?.onClose(() => {
        if (!window.presentationState.closed) window.requestClose();
      });
    if (owner) {
      window.ownershipReady = this.app.services.windows.register({
        window: String(window.id),
        owner: String(owner.id),
        modal,
      });
      window.ownershipReady.catch((error) =>
        this.app.debug?.log?.("error", error.message),
      );
    }
    this.windows.set(key, window);
    this.stops.set(
      key,
      window.onClose(() => {
        stopOwner?.();
        this.dialogs.get(window)?.destroy();
        this.dialogs.delete(window);
        this.windows.delete(key);
        this.stops.delete(key);
      }),
    );
    return window;
  }
  focus(key) {
    return this.app.services.windows.focus({
      window: String(this.get(key).id),
    });
  }
  get(key) {
    const window = this.windows.get(key);
    if (!window || window.presentationState.closed)
      throw Error("Unknown or closed window");
    return window;
  }
  close(key) {
    this.get(key).requestClose();
  }
  dialog(window) {
    if (window?.application !== this.app || window.presentationState.closed)
      throw TypeError("Choose a live app-owned window");
    let controller = this.dialogs.get(window);
    if (!controller)
      this.dialogs.set(window, (controller = new DialogController({ window })));
    return controller;
  }
  dispose() {
    for (const dialog of this.dialogs.values()) dialog.destroy();
    for (const stop of this.stops.values()) stop();
    this.dialogs.clear();
    this.windows.clear();
    this.stops.clear();
  }
}

/** Physical-key shortcuts scoped to a window or retained control subtree. */
export class ShortcutRouter {
  static controlFactory = Object.freeze({
    topics: ["windows", "commands", "workflow"],
    constructor:
      "new ShortcutRouter({scope:window.root,commands:CommandRegistry,bindings:[{command,keyCode,modifiers:4,allowInText:false}]})",
    methods:
      "dispose(); routes pressed native Message.KEY only, honours disabled commands and text editing. Use physical X11 key codes and Shift1/Control4/Alt8/Meta64. Command labels remain presentation.",
  });
  constructor({ scope, commands, bindings = [] } = {}) {
    if (
      !scope?.on ||
      !commands?.execute ||
      !Array.isArray(bindings) ||
      bindings.length > 128
    )
      throw TypeError("Invalid shortcut scope");
    const chords = new Set();
    for (const binding of bindings) {
      commands.get(binding.command);
      if (
        !Number.isInteger(binding.keyCode) ||
        binding.keyCode < 8 ||
        binding.keyCode > 255 ||
        !Number.isInteger(binding.modifiers) ||
        binding.modifiers < 0 ||
        binding.modifiers > 255
      )
        throw TypeError("Invalid shortcut chord");
      const chord = binding.keyCode + ":" + binding.modifiers;
      if (chords.has(chord)) throw TypeError("Duplicate shortcut chord");
      chords.add(chord);
    }
    this.stop = scope.on(Message.KEY, (event) => {
      const detail = event.detail;
      if (!detail?.pressed || event.defaultPrevented) return;
      const binding = bindings.find(
        (b) =>
          b.keyCode === detail.keyCode &&
          b.modifiers === (detail.modifiers & 77),
      );
      if (!binding) return;
      let input = event.target;
      while (input && input !== scope) {
        if (!binding.allowInText && input.family === "input") return;
        input = input.parent;
      }
      const command = commands.get(binding.command);
      if (!command.enabled || !command.visible || command.busy) return;
      event.preventDefault();
      event.stopPropagation();
      return commands.execute(binding.command);
    });
  }
  dispose() {
    this.stop();
  }
}
