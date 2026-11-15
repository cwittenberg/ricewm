import { State } from "./control.mjs";
import {
  Button,
  Menu,
  MenuBar,
  MenuItem,
  ToolBar,
  Tooltip,
} from "./controls.mjs";
import { Message } from "./messages.mjs";
import { Ribbon,ribbonCommandArtwork } from "./ribbon.mjs";

/** One command model, separate native controls for each menu/ribbon surface. */
export class CommandRegistry {
  static controlFactory = Object.freeze({
    topics: ["commands", "product", "visual"],
    constructor:
      "new CommandRegistry([{id,label,run,validate?:async(data,context)=>boolean,arguments?:()=>data,icon?,tooltip?,shortcut?,enabled?,visible?,checked?},...])",
    methods:
      "execute(id,data?,context?), validate(id,data,context?), update(id,{label,enabled,visible,checked,disabledReason}), list({query,includeUnavailable}), subscribe(listener), button(id,properties?), menuItem(id,properties?), menuBar([{label,items:[commandId,{label,items:[...]},null]}]), toolbar([commandId]), ribbon({tabs:[{id,title,groups:[{id,title,commands:[commandId],artwork?:{[commandId]:sceneFrameOrThemeFactory},primary?,rowBreak?,rowCount?:1|2|3,alignColumns?:boolean}]}],...Ribbon options}). CommandPalette searches the same registry. dispose() releases subscriptions without destroying caller-owned surfaces.",
    behavior:
      "Menu trees create native MenuItem -> Menu cascades and separators. Use ShortcutRouter for scoped native key input. Menu and toolbar commands use arguments() when supplied; typed Copilot calls use explicit validated input. validate and run are shared across surfaces. State updates synchronize every surface including overflow. Async commands are disabled while running and restore state on success/failure. Use file menus for lifecycle/export, grouped Ribbon commands for editing, TabView for page navigation. No application command names are imposed.",
  });
  constructor(commands = []) {
    if (!Array.isArray(commands)) throw TypeError("Commands must be an array");
    this.commands = new Map();
    this.bindings = new Map();
    this.listeners = new Set();
    this.disposed = false;
    for (const command of commands) {
      if (
        !command ||
        typeof command.id !== "string" ||
        !/^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(command.id) ||
        !command.label?.trim() ||
        typeof command.run !== "function" ||
        (command.validate !== undefined &&
          typeof command.validate !== "function") ||
        (command.arguments !== undefined &&
          typeof command.arguments !== "function") ||
        this.commands.has(command.id)
      )
        throw TypeError("Commands need unique ids, labels and run functions");
      this.commands.set(command.id, {
        enabled: true,
        visible: true,
        checked: false,
        ...command,
        busy: false,
      });
      this.bindings.set(command.id, new Set());
    }
  }
  get(id) {
    const command = this.commands.get(id);
    if (!command || this.disposed)
      throw RangeError("Unknown or disposed command");
    return command;
  }
  update(id, patch) {
    const command = this.get(id);
    for (const key of Object.keys(patch))
      if (!["label", "enabled", "visible", "checked", "disabledReason"].includes(key))
        throw TypeError("Unsupported command state");
    if (
      Object.hasOwn(patch, "label") &&
      (typeof patch.label !== "string" || !patch.label.trim())
    )
      throw TypeError("Command label is required");
    if (Object.hasOwn(patch, 'disabledReason') && patch.disabledReason !== null && (typeof patch.disabledReason !== 'string' || patch.disabledReason.length > 512)) throw TypeError('Invalid disabled reason');
    Object.assign(command, patch);
    this.sync(id);
  }
  execute(id, data, context) {
    const command = this.get(id);
    if (!command.enabled || !command.visible || command.busy) return false;
    command.busy = true;
    this.sync(id);
    const finish = () => {
      command.busy = false;
      if (!this.disposed) this.sync(id);
    };
    try {
      const arguments_ =
        data === undefined && command.arguments ? command.arguments() : data;
      const run = () => {
        context?.signal?.throwIfAborted();
        if (!command.enabled || !command.visible)
          throw Error("Command is unavailable");
        return command.run(arguments_, context);
      };
      const guarded = () => {
        const guard = context?.guard?.();
        return guard && typeof guard.then === "function"
          ? Promise.resolve(guard).then(run)
          : run();
      };
      const checked = (valid) => {
        if (valid === false)
          throw Error("Command validation rejected this action");
        if (valid !== true && valid !== undefined)
          throw TypeError("Command validation must return true or false");
        return guarded();
      };
      const validation = command.validate
        ? command.validate(arguments_, context)
        : true;
      const result =
        validation && typeof validation.then === "function"
          ? Promise.resolve(validation).then(checked)
          : checked(validation);
      if (result && typeof result.then === "function")
        return Promise.resolve(result).finally(finish);
      finish();
      return result;
    } catch (error) {
      finish();
      throw error;
    }
  }
  async validate(id, data, context) {
    const command = this.get(id);
    if (!command.enabled || !command.visible || command.busy)
      throw Error("Command is unavailable");
    const arguments_ =
        data === undefined && command.arguments ? command.arguments() : data,
      result = command.validate
        ? await command.validate(arguments_, context)
        : true;
    if (result === false)
      throw Error("Command validation rejected this action");
    if (result !== true && result !== undefined)
      throw TypeError("Command validation must return true or false");
    return true;
  }
  sync(id) {
    const command = this.get(id);
    for (const record of this.bindings.get(id)) {
      record.control.text =
        record.menu && command.shortcut
          ? `${command.label}\t${command.shortcut}`
          : command.label;
      record.control.enabled = Boolean(command.enabled && !command.busy);
      record.control.visible = Boolean(command.visible);
      record.control.setProperty("checked", Boolean(command.checked));
    }
    for (const listener of [...this.listeners]) listener(id);
  }
  list({ query = '', includeUnavailable = true } = {}) {
    if (typeof query !== 'string' || query.length > 512) throw TypeError('Invalid command query');
    if (this.disposed) return [];
    const words = query.toLocaleLowerCase().trim().split(/\s+/u).filter(Boolean);
    return [...this.commands.values()].filter(command => command.visible && (includeUnavailable || command.enabled && !command.busy))
      .filter(command => words.every(word => `${command.label} ${command.id} ${command.keywords || ''}`.toLocaleLowerCase().includes(word)))
      .map(command => Object.freeze({ id: command.id, label: command.label, shortcut: command.shortcut || null, icon: command.icon ?? null,
        enabled: Boolean(command.enabled && !command.busy), checked: Boolean(command.checked), disabledReason: command.busy ? 'Command is running' : command.enabled ? null : command.disabledReason || 'Unavailable in the current selection' }));
  }
  subscribe(listener) {
    if (this.disposed || typeof listener !== 'function') throw TypeError('Expected a live command subscription');
    this.listeners.add(listener); return () => this.listeners.delete(listener);
  }
  create(id, menu, properties) {
    const command = this.get(id),
      Constructor = menu ? MenuItem : Button;
    const control = new Constructor({
      name: `command-${id}`,
      icon: command.icon ?? null,
      accessibleName: command.label,
      state: menu ? 0 : State.START_ALIGNED,
      ...properties,
    });
    if (command.tooltip) control.append(new Tooltip({ text: command.tooltip }));
    const off = control.on(Message.ACTIVATE, () => this.execute(id));
    const record = { control, menu, off };
    this.bindings.get(id).add(record);
    record.destroyOff = control.on(Message.DESTROY, () => {
      this.bindings.get(id)?.delete(record);
      off();
    });
    this.sync(id);
    return control;
  }
  button(id, properties = {}) {
    return this.create(id, false, properties);
  }
  menuItem(id, properties = {}) {
    return this.create(id, true, properties);
  }
  menuBar(menus, properties = {}) {
    const items = (entries, depth = 0) => {
      if (!Array.isArray(entries) || depth > 8)
        throw TypeError("Menu items must be a bounded tree");
      return entries.map((entry) => {
        if (entry === null)
          return new MenuItem({ text: "menu:separator", enabled: false });
        if (typeof entry === "string") return this.menuItem(entry);
        if (!entry || typeof entry.label !== "string" || !entry.label.trim())
          throw TypeError("Menu branches need labels");
        return new MenuItem({ text: entry.label }, [
          new Menu({}, items(entry.items, depth + 1)),
        ]);
      });
    };
    return new MenuBar(
      properties,
      menus.map((menu) => new Menu({ text: menu.label }, items(menu.items))),
    );
  }
  toolbar(ids, properties = {}) {
    return new ToolBar(
      properties,
      ids.map((id) => this.button(id)),
    );
  }
  ribbon({ tabs = [], ...properties } = {}) {
    return new Ribbon({
      presentation: "office",
      ...properties,
      tabs: tabs.map((tab) => ({
        ...tab,
        groups: tab.groups.map(({ commands, artwork, ...group }) => ({
          ...group,
          controls: commands.map((id) => {const button=this.button(id);return artwork&&Object.hasOwn(artwork,id)?ribbonCommandArtwork(button,artwork[id]):button;}),
        })),
      })),
    });
  }
  dispose() {
    for (const records of this.bindings.values())
      for (const record of records) {
        record.off();
        record.destroyOff();
      }
    this.bindings.clear();
    this.listeners.clear();
    this.disposed = true;
  }
}
