export const MAX_NATIVE_ARGUMENTS = 16;
export const MAX_NATIVE_ARGUMENT_BYTES = 16 * 1024;

const WIRE_CLOSE_REASONS = new Set([
    "requested",
    "last-window-closed",
    "host-shutdown",
    "protocol-fault",
]);

const ENTRYPOINTS = Object.freeze({
    "control-gallery": "RiceWM UI Library",
    "rice-calc": "Calc",
    "rice-cava": "CAVA (Wasm)",
    "rice-java-bundle": "Java Wasm application",
    "rice-desktop": "Desktop",
    "rice-day": "Day",
    "rice-box": "Box",
    "rice-card": "Card",
    "rice-paper": "Paper",
    "rice-paddy": "Paddy",
    "rice-files": "Files",
    "rice-host-surface": "Application",
    "rice-player": "Player",
    "rice-performance": "System Monitor",
    "rice-profiler": "Profiler",
    "rice-setup": "APT App Store",
    "rice-store": "Rice Store",
    "rice-extensions": "Extension Manager",
    "rice-settings": "Settings",
    "rice-terminal": "Terminal",
    "rice-term": "Terminal",
    "rice-view": "View",
    "rice-welcome": "Welcome",
    "rice-sdk-app": "RiceApp",
    "bowl": "Bowl",
});

const SYSTEM_ENTRYPOINTS = Object.freeze({
    "rice-screenshot": "Screenshot",
});

const ENTRYPOINT_ICONS = Object.freeze({
    "control-gallery": "view-grid",
    "rice-calc": "calculator",
    "rice-cava": "media-play",
    "rice-java-bundle": "terminal",
    "rice-desktop": "desktop",
    "rice-day": "calendar",
    "rice-box": "inbox",
    "rice-card": "user",
    "rice-paper": "text-editor",
    "rice-paddy": "comment",
    "rice-files": "files",
    "rice-host-surface": "desktop",
    "rice-player": "media-play",
    "rice-performance": "system-monitor",
    "rice-profiler": "search",
    "rice-setup": "object-add",
    "rice-store": "ricewm-logo",
    "rice-extensions": "welcome-extensions",
    "rice-settings": "settings",
    "rice-terminal": "terminal",
    "rice-term": "terminal",
    "rice-view": "image-viewer",
    "rice-welcome": "ricewm-logo",
    "rice-sdk-app": "program",
    "bowl": "bowl",
    "rice-screenshot": "camera",
});

export const NATIVE_ENTRYPOINTS = Object.freeze(Object.keys(ENTRYPOINTS));

export function nativeEntrypointTitle(entrypoint) {
    if (!Object.hasOwn(ENTRYPOINTS, entrypoint) &&
        !Object.hasOwn(SYSTEM_ENTRYPOINTS, entrypoint)) {
        throw new RangeError("native application entrypoint is not allowed");
    }
    return ENTRYPOINTS[entrypoint] || SYSTEM_ENTRYPOINTS[entrypoint];
}

export function nativeEntrypointIconName(entrypoint) {
    return ENTRYPOINT_ICONS[entrypoint] || "desktop";
}

export function checkedNativeLaunch(entrypoint, arguments_) {
    nativeEntrypointTitle(entrypoint);
    if (
        !Array.isArray(arguments_) ||
        arguments_.length > MAX_NATIVE_ARGUMENTS
    ) {
        throw new RangeError("native application argument limit exceeded");
    }
    const argumentsCopy = arguments_.map(argument => String(argument));
    const bytes = new TextEncoder().encode(argumentsCopy.join("\0"));
    if (bytes.byteLength > MAX_NATIVE_ARGUMENT_BYTES) {
        throw new RangeError("native application arguments are oversized");
    }
    return argumentsCopy;
}

export function nativeWireCloseReason(reason, fallback) {
    return WIRE_CLOSE_REASONS.has(reason) ? reason : fallback;
}
