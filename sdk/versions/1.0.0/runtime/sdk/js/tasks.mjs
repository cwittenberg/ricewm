import { createCancellationScope } from "./cancellation.mjs";

/** App-owned asynchronous work. Superseded results never replace newer state. */
export class TaskController {
  static controlFactory = Object.freeze({
    topics: ["workflow", "product"],
    constructor: 'new TaskController({mode:"reject"|"queue"|"latest"})',
    methods:
      "run(async({signal,progress})=>result), cancel(), subscribe(listener), close(); state is {status,progress,result,error}. No implicit retries of mutations.",
  });
  constructor({ mode = "reject" } = {}) {
    if (!["reject", "queue", "latest"].includes(mode))
      throw TypeError("Invalid task mode");
    this.mode = mode;
    this.listeners = new Set();
    this.tail = Promise.resolve();
    this.serial = 0;
    this.closed = false;
    this.state = Object.freeze({ status: "idle", progress: null });
  }
  publish(state) {
    this.state = Object.freeze(state);
    for (const listener of [...this.listeners]) listener(this.state);
  }
  subscribe(listener) {
    if (typeof listener !== "function")
      throw TypeError("Expected task listener");
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  run(work) {
    if (this.closed || typeof work !== "function")
      return Promise.reject(Error("Task is closed or invalid"));
    if (this.mode === "reject" && this.state.status === "running")
      return Promise.reject(
        Object.assign(Error("Task is busy"), { code: "BUSY" }),
      );
    if (this.mode === "latest") this.cancel();
    const serial = ++this.serial,
      execute = async () => {
        if (this.closed || (this.mode === "latest" && serial !== this.serial))
          throw Object.assign(Error("Task superseded"), { code: "CANCELLED" });
        const scope = createCancellationScope();
        this.scope = scope;
        this.publish({ status: "running", progress: null });
        const progress = (value) => {
          scope.signal.throwIfAborted();
          if (this.scope === scope)
            this.publish({
              status: "running",
              progress: structuredClone(value),
            });
        };
        try {
          const result = await work({ signal: scope.signal, progress });
          scope.signal.throwIfAborted();
          if (this.scope === scope)
            this.publish({
              status: "succeeded",
              progress: this.state.progress,
              result,
            });
          return result;
        } catch (error) {
          if (this.scope === scope)
            this.publish({
              status: scope.signal.aborted ? "cancelled" : "failed",
              progress: this.state.progress,
              error,
            });
          throw error;
        } finally {
          if (this.scope === scope) this.scope = null;
        }
      };
    const pending =
      this.mode === "queue"
        ? this.tail.catch(() => {}).then(execute)
        : execute();
    this.tail = pending;
    return pending;
  }
  cancel() {
    this.scope?.abort(
      Object.assign(Error("Task cancelled"), { code: "CANCELLED" }),
    );
  }
  close() {
    this.closed = true;
    this.serial++;
    this.cancel();
    this.listeners.clear();
  }
}

/** Serialize bounded worker resources; callers retain ownership of documents. */
export class ResourcePool {
  static controlFactory = Object.freeze({
    topics: ["workflow", "documents", "media"],
    constructor:
      "new ResourcePool({maximum:4,create:async(key)=>resource,destroy:async(resource)=>{...}})",
    methods:
      "acquire(key,{signal}), close(); acquire resolves {resource,release}. Leases are exclusive; release in finally. Pending requests are bounded.",
  });
  constructor({
    maximum = 4,
    create,
    destroy = (resource) => resource.close?.(),
  } = {}) {
    if (
      !Number.isInteger(maximum) ||
      maximum < 1 ||
      maximum > 32 ||
      typeof create !== "function" ||
      typeof destroy !== "function"
    )
      throw TypeError("Invalid resource pool");
    Object.assign(this, { maximum, create, destroy });
    this.entries = [];
    this.waiting = [];
    this.creating = 0;
    this.closed = false;
  }
  acquire(key, { signal } = {}) {
    if (this.closed || signal?.aborted)
      return Promise.reject(Error("Resource request stopped"));
    if (this.waiting.length >= 128)
      return Promise.reject(Error("Resource queue full"));
    return new Promise((resolve, reject) => {
      const request = { key, signal, resolve, reject };
      request.abort = () => {
        const index = this.waiting.indexOf(request);
        if (index >= 0) {
          this.waiting.splice(index, 1);
          reject(Error("Resource request cancelled"));
        }
      };
      signal?.addEventListener("abort", request.abort, { once: true });
      this.waiting.push(request);
      void this.pump();
    });
  }
  async pump() {
    while (!this.closed && this.waiting.length) {
      const request = this.waiting[0];
      let entry = this.entries.find((e) => !e.busy && e.key === request.key);
      if (!entry) {
        if (this.creating) return;
        const stale = this.entries.find((e) => !e.busy);
        if (this.entries.length + this.creating >= this.maximum && !stale)
          return;
        this.waiting.shift();
        this.creating++;
        try {
          if (stale) {
            this.entries.splice(this.entries.indexOf(stale), 1);
            await this.destroy(stale.resource);
          }
          const resource = await this.create(request.key);
          if (this.closed || request.signal?.aborted) {
            await this.destroy(resource);
            throw Error("Resource request stopped");
          }
          entry = { key: request.key, resource, busy: false };
          this.entries.push(entry);
        } catch (error) {
          request.signal?.removeEventListener("abort", request.abort);
          request.reject(error);
          continue;
        } finally {
          this.creating--;
        }
      } else this.waiting.shift();
      request.signal?.removeEventListener("abort", request.abort);
      entry.busy = true;
      let released = false;
      request.resolve({
        resource: entry.resource,
        release: () => {
          if (released) return;
          released = true;
          entry.busy = false;
          if (this.closed) {
            const index = this.entries.indexOf(entry);
            if (index >= 0) this.entries.splice(index, 1);
            return this.destroy(entry.resource);
          } else void this.pump();
        },
      });
    }
  }
  async close() {
    if (this.closed) return;
    this.closed = true;
    for (const request of this.waiting.splice(0)) {
      request.signal?.removeEventListener("abort", request.abort);
      request.reject(Error("Resource pool closed"));
    }
    const idle = this.entries.filter((e) => !e.busy);
    this.entries = this.entries.filter((e) => e.busy);
    await Promise.all(idle.map((e) => this.destroy(e.resource)));
  }
}
