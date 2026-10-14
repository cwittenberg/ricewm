const MAX_PATH_BYTES = 4 * 1024;
const encoder = new TextEncoder();

export class RiceGuestPathError extends Error {
    constructor(message) {
        super(message);
        this.name = "RiceGuestPathError";
    }
}

function checkedText(value, field) {
    if (typeof value !== "string") {
        throw new RiceGuestPathError(`${field} must be a string`);
    }
    if (encoder.encode(value).byteLength > MAX_PATH_BYTES) {
        throw new RiceGuestPathError(
            `${field} exceeds the ${MAX_PATH_BYTES}-byte guest-path limit`,
        );
    }
    for (let index = 0; index < value.length; index += 1) {
        const unit = value.charCodeAt(index);
        if (unit === 0 || unit < 0x20 || unit === 0x7f) {
            throw new RiceGuestPathError(
                `${field} contains a control character`,
            );
        }
        if (unit >= 0xd800 && unit <= 0xdbff) {
            const next = value.charCodeAt(index + 1);
            if (!(next >= 0xdc00 && next <= 0xdfff)) {
                throw new RiceGuestPathError(
                    `${field} contains an unpaired surrogate`,
                );
            }
            index += 1;
        } else if (unit >= 0xdc00 && unit <= 0xdfff) {
            throw new RiceGuestPathError(
                `${field} contains an unpaired surrogate`,
            );
        }
    }
    return value;
}

/**
 * Canonicalizes one absolute POSIX guest path without consulting host state.
 *
 * Parent traversal above `/` is rejected instead of silently clamped. This
 * keeps a future mount/provider boundary explicit.
 */
export function normalizeGuestPath(value) {
    const path = checkedText(value, "guest path");
    if (!path.startsWith("/")) {
        throw new RiceGuestPathError("guest path must be absolute");
    }
    const components = [];
    for (const component of path.split("/")) {
        if (component === "" || component === ".") continue;
        if (component === "..") {
            if (components.length === 0) {
                throw new RiceGuestPathError(
                    "guest path attempts to traverse above root",
                );
            }
            components.pop();
        } else {
            components.push(component);
        }
    }
    return components.length === 0 ? "/" : `/${components.join("/")}`;
}

/** Validates a single directory-entry basename returned by a host provider. */
export function normalizeGuestEntryName(value) {
    const name = checkedText(value, "directory entry name");
    if (
        name.length === 0 ||
        name === "." ||
        name === ".." ||
        name.includes("/")
    ) {
        throw new RiceGuestPathError(
            "directory entry name must be one non-special basename",
        );
    }
    return name;
}

export const RICE_GUEST_PATH_MAX_BYTES = MAX_PATH_BYTES;
