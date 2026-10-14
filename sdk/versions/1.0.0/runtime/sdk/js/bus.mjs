import { Message } from "./messages.mjs";

const BUS_NAME = /^[a-z0-9](?:[a-z0-9.-]{0,62})(?:\/[a-z0-9](?:[a-z0-9._-]{0,62}))*$/;

function checkedName(value, field) {
    if (typeof value !== "string" || !BUS_NAME.test(value) || value.length > 255) {
        throw new TypeError(`${field} is not a valid RiceBus name`);
    }
    return value;
}

const MAX_PAYLOAD_BYTES = 256 * 1024;
const MAX_PAYLOAD_ENTRIES = 4096;

export function copyRiceBusPayload(value) {
    let entries = 0;
    let bytes = 0;
    const seen = new Set();
    const visit = (current, depth) => {
        if (depth > 32 || ++entries > MAX_PAYLOAD_ENTRIES) {
            throw new RangeError("RiceBus payload is too complex");
        }
        if (current === null || typeof current === "boolean") return current;
        if (typeof current === "number") {
            if (!Number.isFinite(current)) throw new TypeError("RiceBus numbers must be finite");
            return current;
        }
        if (typeof current === "string") {
            bytes += new TextEncoder().encode(current).byteLength;
            if (bytes > MAX_PAYLOAD_BYTES) throw new RangeError("RiceBus payload exceeds 256 KiB");
            return current;
        }
        if (current instanceof Uint8Array) {
            bytes += current.byteLength;
            if (bytes > MAX_PAYLOAD_BYTES) throw new RangeError("RiceBus payload exceeds 256 KiB");
            return current.slice();
        }
        if (typeof current !== "object" || seen.has(current)) {
            throw new TypeError("RiceBus payload must be acyclic data");
        }
        seen.add(current);
        let result;
        if (Array.isArray(current)) {
            result = Object.freeze(current.map(item => visit(item, depth + 1)));
        } else {
            const prototype = Object.getPrototypeOf(current);
            if (prototype !== Object.prototype && prototype !== null) {
                throw new TypeError("RiceBus payload objects must be plain data");
            }
            result = Object.freeze(Object.fromEntries(
                Object.entries(current).map(([key, item]) => {
                    bytes += new TextEncoder().encode(key).byteLength;
                    if(bytes>MAX_PAYLOAD_BYTES)throw new RangeError('RiceBus payload exceeds 256 KiB');
                    return [key, visit(item, depth + 1)];
                }),
            ));
        }
        seen.delete(current);
        return result;
    };
    return visit(value, 0);
}

export class RiceBusBackbone {
    constructor() {
        this.subscriptions = new Map();
        this.services = new Map();
    }

    subscribe(topic, owner, listener) {
        checkedName(topic, "topic");
        const subscriptions = this.subscriptions.get(topic) || new Set();
        const subscription = Object.freeze({ owner, listener });
        subscriptions.add(subscription);
        this.subscriptions.set(topic, subscriptions);
        return () => subscriptions.delete(subscription);
    }

    publish(topic, owner, payload) {
        const event = Object.freeze({ type: Message.BUS_EVENT, topic, source: owner, payload: copyRiceBusPayload(payload) });
        queueMicrotask(() => {
            for (const subscription of [...(this.subscriptions.get(topic) || [])]) {
                subscription.listener(Object.freeze({ ...event, payload: copyRiceBusPayload(event.payload) }));
            }
        });
    }

    expose(name, owner, handler) {
        checkedName(name, "service");
        if (this.services.has(name)) throw new Error(`RiceBus service ${name} is already owned`);
        const service = Object.freeze({ owner, handler });
        this.services.set(name, service);
        return () => {
            if (this.services.get(name) === service) this.services.delete(name);
        };
    }

    async call(name, owner, request) {
        const service = this.services.get(name);
        if (!service) throw new Error(`RiceBus service ${name} is unavailable`);
        return copyRiceBusPayload(await service.handler(Object.freeze({
            type: Message.BUS_CALL,
            service: name,
            source: owner,
            request: copyRiceBusPayload(request),
        })));
    }
}

export class RiceBus {
    #backbone;
    #owner;
    #permissions;
    constructor({ owner, backbone, permissions = {} }) {
        this.#owner = checkedName(owner, "application bus name");
        if (!backbone || ["subscribe", "publish", "expose", "call"].some(
            method => typeof backbone[method] !== "function"
        )) throw new TypeError("RiceBus backbone is required");
        this.#backbone = backbone;
        this.#permissions = Object.freeze({
            publish: Object.freeze([...(permissions.publish || [`${owner}/`])]),
            subscribe: Object.freeze([...(permissions.subscribe || [])]),
            provide: Object.freeze([...(permissions.provide || [`${owner}/`])]),
            call: Object.freeze([...(permissions.call || [])]),
        });
        this.releases = new Set();
    }

    get owner() { return this.#owner; }
    get permissions() { return this.#permissions; }

    #allowed(kind, name) {
        const prefixes = this.#permissions[kind];
        return prefixes.includes("*") || prefixes.some(prefix => name === prefix || name.startsWith(prefix));
    }

    #require(kind, name) {
        checkedName(name, kind);
        if (!this.#allowed(kind, name)) throw new Error(`${this.#owner} cannot ${kind} ${name}`);
    }

    publish(topic, payload = null) {
        this.#require("publish", topic);
        this.#backbone.publish(topic, this.#owner, payload);
    }

    subscribe(topic, listener) {
        this.#require("subscribe", topic);
        if (typeof listener !== "function") throw new TypeError("subscriber must be a function");
        const release = this.#backbone.subscribe(topic, this.#owner, listener);
        this.releases.add(release);
        return () => { release(); this.releases.delete(release); };
    }

    expose(name, handler) {
        this.#require("provide", name);
        if (typeof handler !== "function") throw new TypeError("service handler must be a function");
        const release = this.#backbone.expose(name, this.#owner, handler);
        this.releases.add(release);
        return () => { release(); this.releases.delete(release); };
    }

    call(name, request = null) {
        this.#require("call", name);
        return this.#backbone.call(name, this.#owner, request);
    }

    close() {
        for (const release of this.releases) release();
        this.releases.clear();
    }
}
