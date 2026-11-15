import { desktopEventTopics } from "./desktop.mjs";
import { TaskController } from "./tasks.mjs";
const plain = (v) =>
  v !== null &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.getPrototypeOf(v) === Object.prototype;
const text = (v, max = 2048) =>
  typeof v === "string" && v.trim() && v.length <= max && !v.includes("\0");
const types = new Set([
  "object",
  "array",
  "string",
  "number",
  "integer",
  "boolean",
  "null",
]);
/** Deliberately bounded JSON Schema subset, shared by package and runtime. */
export function validateCapabilitySchema(schema, depth = 0) {
  if (
    !plain(schema) ||
    depth > 8 ||
    !types.has(schema.type) ||
    Object.keys(schema).some(
      (k) =>
        ![
          "type",
          "properties",
          "required",
          "additionalProperties",
          "items",
          "enum",
          "minimum",
          "maximum",
          "minLength",
          "maxLength",
          "maxItems",
          "description",
        ].includes(k),
    )
  )
    throw TypeError("Unsupported capability schema");
  if (schema.description !== undefined && !text(schema.description))
    throw TypeError("Invalid schema description");
  if (schema.type === "object") {
    if (
      !plain(schema.properties) ||
      Object.keys(schema.properties).length > 64 ||
      schema.additionalProperties !== false
    )
      throw TypeError(
        "Object schemas need bounded properties and additionalProperties:false",
      );
    for (const [key, value] of Object.entries(schema.properties)) {
      if (!/^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(key))
        throw TypeError("Invalid schema property");
      validateCapabilitySchema(value, depth + 1);
    }
    if (
      schema.required !== undefined &&
      (!Array.isArray(schema.required) ||
        new Set(schema.required).size !== schema.required.length ||
        schema.required.some((k) => !Object.hasOwn(schema.properties, k)))
    )
      throw TypeError("Invalid required properties");
  }
  if (schema.type === "array") {
    if (
      !Number.isInteger(schema.maxItems) ||
      schema.maxItems < 0 ||
      schema.maxItems > 4096
    )
      throw TypeError("Arrays need maxItems 0..4096");
    validateCapabilitySchema(schema.items, depth + 1);
  }
  for (const k of ["minLength", "maxLength"])
    if (
      schema[k] !== undefined &&
      (!Number.isInteger(schema[k]) || schema[k] < 0 || schema[k] > 65536)
    )
      throw TypeError("Invalid string bounds");
  for (const k of ["minimum", "maximum"])
    if (
      schema[k] !== undefined &&
      (typeof schema[k] !== "number" || !Number.isFinite(schema[k]))
    )
      throw TypeError("Invalid number bounds");
  if (
    schema.enum !== undefined &&
    (!Array.isArray(schema.enum) ||
      !schema.enum.length ||
      schema.enum.length > 128 ||
      schema.enum.some(
        (v) =>
          (v !== null && typeof v === "object") ||
          typeof v === "function" ||
          typeof v === "undefined",
      ))
  )
    throw TypeError("Invalid enum");
  return schema;
}
export function validateCapabilityValue(schema, value, depth = 0) {
  if (depth === 0) {
    validateCapabilitySchema(schema);
    const encoded = JSON.stringify(value);
    if (
      encoded === undefined ||
      new TextEncoder().encode(encoded).length > 262144
    )
      throw RangeError("Capability value exceeds 256 KiB");
  }
  if (schema.type === "object") {
    if (
      !plain(value) ||
      Object.keys(value).some((k) => !Object.hasOwn(schema.properties, k)) ||
      (schema.required || []).some((k) => !Object.hasOwn(value, k))
    )
      throw TypeError("Invalid capability object");
    for (const [k, v] of Object.entries(value))
      validateCapabilityValue(schema.properties[k], v, depth + 1);
  } else if (schema.type === "array") {
    if (!Array.isArray(value) || value.length > schema.maxItems)
      throw TypeError("Invalid capability array");
    for (const item of value)
      validateCapabilityValue(schema.items, item, depth + 1);
  } else if (
    schema.type === "null"
      ? value !== null
      : schema.type === "integer"
        ? !Number.isSafeInteger(value)
        : typeof value !== schema.type ||
          (schema.type === "number" && !Number.isFinite(value))
  )
    throw TypeError("Invalid capability value type");
  if (
    typeof value === "string" &&
    (value.length < (schema.minLength ?? 0) ||
      value.length > (schema.maxLength ?? 65536))
  )
    throw RangeError("Capability string is outside bounds");
  if (
    typeof value === "number" &&
    (value < (schema.minimum ?? -Infinity) ||
      value > (schema.maximum ?? Infinity))
  )
    throw RangeError("Capability number is outside bounds");
  if (schema.enum && !schema.enum.some((v) => Object.is(v, value)))
    throw TypeError("Value is outside capability enum");
  return value;
}
export function validateAppCapabilities(descriptors = []) {
  if (!Array.isArray(descriptors) || descriptors.length > 64)
    throw TypeError("At most 64 app capabilities may be declared");
  const ids = new Set();
  return Object.freeze(
    descriptors.map((d) => {
      if (
        !plain(d) ||
        Object.keys(d).some(
          (k) =>
            ![
              "id",
              "title",
              "description",
              "input",
              "output",
              "effect",
              "undoable",
            ].includes(k),
        ) ||
        !text(d.id, 96) ||
        !/^[A-Za-z][A-Za-z0-9_.-]*$/.test(d.id) ||
        ids.has(d.id) ||
        !text(d.title, 128) ||
        !text(d.description) ||
        !["query", "mutation", "external"].includes(d.effect) ||
        typeof d.undoable !== "boolean"
      )
        throw TypeError("Invalid app capability descriptor");
      validateCapabilitySchema(d.input);
      validateCapabilitySchema(d.output);
      ids.add(d.id);
      return Object.freeze(structuredClone(d));
    }),
  );
}

/** Protected, exact-window semantic AI access. Uses the same commands as UI. */
export class AppCapabilityRegistry {
  static controlFactory = Object.freeze({
    topics: ["copilot", "workflow", "product"],
    constructor: "new AppCapabilityRegistry({app,bus,services})",
    methods:
      "bind({window,context:async()=>({revision,ready,document}),commands:CommandRegistry,bindings:{capabilityId:commandId}}). Returns async dispose. Declare typed manifest.capabilities and capabilities.register/respond/unregister permissions, plus the exact desktopEventTopics(app.manifest.id).capabilityInvoked subscribe topic. All declared capabilities must bind. UI and Copilot invoke the same command handler, with schema validation and stale-revision rejection. No Bus provide/call authority.",
  });
  constructor({ app, bus, services } = {}) {
    if (!app || !bus || !services?.capabilities)
      throw TypeError("App capabilities need an SDK app, bus and services");
    Object.assign(this, { app, bus, services });
    this.descriptors = validateAppCapabilities(app.manifest.capabilities);
    this.endpoints = new Map();
    this.stop = bus.subscribe(
      desktopEventTopics(app.manifest.id).capabilityInvoked,
      (event) => this.invoke(event.payload),
    );
    (app.capabilityRegistries ||= new Set()).add(this);
  }
  async bind({ window, context, commands, bindings = {}, onAction } = {}) {
    if (
      window?.application !== this.app ||
      window.presentationState.closed ||
      typeof context !== "function" ||
      typeof commands?.execute !== "function" ||
      this.endpoints.has(String(window.id))
    )
      throw TypeError("Invalid capability window");
    if (
      Object.keys(bindings).length !== this.descriptors.length ||
      this.descriptors.some((d) => !Object.hasOwn(bindings, d.id))
    )
      throw TypeError("Bind every declared app capability");
    for (const command of Object.values(bindings)) commands.get(command);
    const hub =
      this.app.manifest.permissions?.services?.includes("actions.register") &&
      !this.hub;
    if (hub && typeof onAction !== "function")
      throw TypeError(
        "App action registration requires onAction to open the app command workflow",
      );
    const id = String(window.id),
      record = {
        window,
        context,
        commands,
        bindings,
        task: new TaskController(),
      },
      actionId = "app-capabilities";
    this.endpoints.set(id, record);
    try {
      await this.services.capabilities.register({
        window: id,
        ids: this.descriptors.map((d) => d.id),
      });
      if (hub) {
        const stop = this.bus.subscribe(
          desktopEventTopics(this.app.manifest.id).actionInvoked,
          (event) => {
            const current = this.endpoints.values().next().value;
            if (event.payload?.id === actionId && current)
              return onAction({
                window: current.window,
                commands: current.commands,
                capabilities: this.descriptors,
              });
          },
        );
        this.hub = { stop };
        try {
          await this.services.actions.register({
            id: actionId,
            title: (this.app.manifest.name + " commands")
              .replace(/[\x00-\x1f]/g, " ")
              .slice(0, 80),
          });
        } catch (error) {
          stop();
          this.hub = null;
          throw error;
        }
      }
    } catch (error) {
      this.endpoints.delete(id);
      record.task.close();
      await this.services.capabilities
        .unregister({ window: id })
        .catch(() => {});
      throw error;
    }
    let disposed = false;
    const dispose = async () => {
      if (disposed) return;
      disposed = true;
      this.endpoints.delete(id);
      record.task.close();
      record.stopAction?.();
      stopClose();
      await this.services.capabilities.unregister({ window: id });
      if (!this.endpoints.size && this.hub) {
        this.hub.stop();
        this.hub = null;
        await this.services.actions.unregister({ id: actionId });
      }
    };
    record.dispose = dispose;
    const stopClose = window.onClose(() => {
      void dispose().catch(() => {});
    });
    return dispose;
  }
  async invoke(event) {
    const record = this.endpoints.get(event?.window);
    if (!record) return;
    let value, error;
    try {
      value = await record.task.run(async ({ signal }) => {
        const snapshot = await record.context();
        if (
          !snapshot ||
          !["string", "number"].includes(typeof snapshot.revision) ||
          typeof snapshot.ready !== "boolean"
        )
          throw TypeError("Capability context needs revision and ready");
        const actions = Object.fromEntries(
          this.descriptors
            .filter((d) => {
              const command = record.commands.get(record.bindings[d.id]);
              return command.enabled && command.visible && !command.busy;
            })
            .map((d) => [
              d.id,
              {
                description: d.description,
                input: d.input,
                output: d.output,
                effect: d.effect,
                undoable: d.undoable,
              },
            ]),
        );
        const request = event.request;
        if (request?.operation === "context")
          return {
            app: this.app.manifest.id,
            name: record.window.title,
            provider: "protected-app-capabilities",
            ...snapshot,
            actions,
          };
        if (
          !["validate", "act"].includes(request?.operation) ||
          !snapshot.ready ||
          snapshot.revision !== request.expectedRevision
        )
          throw Error("The document changed or is busy; inspect it again");
        const descriptor = this.descriptors.find(
          (d) => d.id === request.action,
        );
        if (!descriptor || !Object.hasOwn(actions, descriptor.id))
          throw Error("Capability is unavailable");
        validateCapabilityValue(descriptor.input, request.data);
        const guard = async () => {
          const fresh = await record.context();
          if (fresh.revision !== snapshot.revision || fresh.ready !== true)
            throw Error("The document changed during validation");
          signal.throwIfAborted();
        };
        if (request.operation === "validate") {
          await record.commands.validate(
            record.bindings[descriptor.id],
            request.data,
            { signal, window: record.window },
          );
          await guard();
          return { valid: true, revision: snapshot.revision };
        }
        const result = await record.commands.execute(
          record.bindings[descriptor.id],
          request.data,
          { signal, window: record.window, guard },
        );
        validateCapabilityValue(descriptor.output, result);
        return result;
      });
    } catch (cause) {
      error = String(cause.message || cause).slice(0, 2048);
    }
    await this.services.capabilities.respond({
      invocation: event.invocation,
      ...(error ? { ok: false, error } : { ok: true, value }),
    });
  }
  async close() {
    this.stop();
    const results = await Promise.allSettled(
      [...this.endpoints.values()].map((record) => record.dispose()),
    );
    this.hub?.stop();
    this.hub = null;
    this.app.capabilityRegistries?.delete(this);
    return results;
  }
}
