const LEVELS = Object.freeze({ trace: 10, debug: 20, info: 30, warn: 40, error: 50 });
const SECRET = /(?:authorization|cookie|credential|password|secret|token|private.?key)/iu;
const MAX_RECORDS = 512;

function safeValue(value, depth = 0, seen = new Set()) {
    if (depth > 8) return "[depth-limit]";
    if (value === null || ["boolean", "number", "string"].includes(typeof value)) {
        return typeof value === "string" ? value.slice(0, 4096) : value;
    }
    if (typeof value === "bigint") return value.toString();
    if (value instanceof Error) return Object.freeze({ name: value.name, message: value.message.slice(0, 2048) });
    if (typeof value !== "object" || seen.has(value)) return `[${typeof value}]`;
    seen.add(value);
    let output;
    if (Array.isArray(value)) output = value.slice(0, 128).map(item => safeValue(item, depth + 1, seen));
    else output = Object.fromEntries(Object.entries(value).slice(0, 128).map(([key, item]) => [
        key,
        SECRET.test(key) ? "[redacted]" : safeValue(item, depth + 1, seen),
    ]));
    seen.delete(value);
    return Object.freeze(output);
}

export class DebugSession {
    constructor({ appId, sink = null, minimumLevel = "debug" } = {}) {
        this.appId = String(appId || "unknown");
        this.sink = typeof sink === "function" ? sink : null;
        this.minimumLevel = LEVELS[minimumLevel] || LEVELS.debug;
        this.records = [];
        this.messageTracing = false;
        this.sequence = 0;
        this.application = null;
    }

    attach(application) { this.application = application; return this; }

    log(level, message, fields = {}) {
        if (!Object.hasOwn(LEVELS, level)) throw new TypeError("unknown debug level");
        if (LEVELS[level] < this.minimumLevel) return null;
        const record = Object.freeze({
            sequence: ++this.sequence,
            timestamp: Date.now(),
            appId: this.appId,
            level,
            message: String(message).slice(0, 2048),
            fields: safeValue(fields),
        });
        this.records.push(record);
        if (this.records.length > MAX_RECORDS) this.records.shift();
        if (this.sink) Promise.resolve(this.sink(record)).catch(() => {});
        return record;
    }

    trace(message, fields) { return this.log("trace", message, fields); }
    debug(message, fields) { return this.log("debug", message, fields); }
    info(message, fields) { return this.log("info", message, fields); }
    warn(message, fields) { return this.log("warn", message, fields); }
    error(message, fields) { return this.log("error", message, fields); }

    span(name, fields = {}) {
        const started = globalThis.performance?.now?.() ?? Date.now();
        let ended = false;
        return Object.freeze({
            end: extra => {
                if (ended) return null;
                ended = true;
                const finished = globalThis.performance?.now?.() ?? Date.now();
                return this.debug(String(name), { ...fields, ...extra, durationMs: Math.max(0, finished - started) });
            },
        });
    }

    assert(condition, message = "assertion failed", fields = {}) {
        if (condition) return true;
        this.error(message, fields);
        throw new Error(String(message));
    }

    traceMessages(enabled = true) { this.messageTracing = Boolean(enabled); }

    recordControlMessage(control, current) {
        if (!this.messageTracing) return;
        this.trace("control-message", {
            control: control.name || control.type,
            controlId: control.id,
            message: current.type,
            source: current.source?.name || current.source?.type || null,
        });
    }

    snapshot() {
        return Object.freeze({
            appId: this.appId,
            controls: this.application?.controlSnapshot?.() || Object.freeze([]),
            logs: Object.freeze(this.records.map(record => record)),
        });
    }
}

export function createLogger(debug, namespace) {
    const prefix = String(namespace || "app");
    return Object.freeze(Object.fromEntries(Object.keys(LEVELS).map(level => [
        level,
        (message, fields) => debug.log(level, `${prefix}: ${message}`, fields),
    ])));
}

/** Console surface installed inside one app realm by the QuickJS host. */
export function createAppConsole(debug) {
    const timers = new Map();
    const write = level => (...values) => {
        const [first, ...rest] = values;
        return debug.log(level, typeof first === "string" ? first : "console", {
            ...(typeof first === "string" ? {} : { value: first }),
            ...(rest.length ? { arguments: rest } : {}),
        });
    };
    return Object.freeze({
        log: write("info"), info: write("info"), debug: write("debug"),
        warn: write("warn"), error: write("error"), trace: write("trace"),
        assert(condition, ...values) {
            if (!condition) write("error")(...(values.length ? values : ["Assertion failed"]));
        },
        time(label = "default") {
            if (timers.size < 128) timers.set(String(label), globalThis.performance?.now?.() ?? Date.now());
        },
        timeEnd(label = "default") {
            label = String(label);
            const started = timers.get(label);
            if (started === undefined) return;
            timers.delete(label);
            const ended = globalThis.performance?.now?.() ?? Date.now();
            debug.debug(`${label}: ${Math.max(0, ended - started).toFixed(3)} ms`);
        },
    });
}
