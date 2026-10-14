import {
    appendWindowSilhouette,
    RiceNativeCanvasWindow,
    riceWindowTransitionDuration,
} from "../rice-native-canvas-window.mjs";
export { riceWindowTransitionDuration };
import { FrostedIceMaterial } from "../rice-frosted-ice.mjs";
import { RiceGnomeEffectRenderer } from "../rice-gnome-effect-renderer.mjs";
import {
    riceIconName,
} from "../rice-native-icon-renderer.mjs";
import {
    NATIVE_ENTRYPOINTS,
    nativeEntrypointIconName,
    nativeEntrypointTitle,
} from "../rice-native-launch-policy.mjs";
import { nativeTitlebarLayout } from "../rice-native-window-stack.mjs";

export const RICE_SCENE_COMMAND_BYTES = 32;
export const RICE_SCENE_RESET = 0;
export const RICE_SCENE_UPSERT_WINDOW = 1;
export const RICE_SCENE_REMOVE_WINDOW = 2;
export const RICE_SCENE_DAMAGE_WINDOW = 3;
export const RICE_NATIVE_MEDIA_CACHE_MAX_BYTES = 200 * 1024 * 1024;
export const RICE_SCENE_STACK_INDEX_MASK = 0x3fff;
export const RICE_SCENE_OWNED_WINDOW_STACK_FLAG = 0x8000;
export const RICE_SCENE_RESIZABLE_WINDOW_STACK_FLAG = 0x4000;

export function riceWmBarBorderPixels(value) {
    const percentage = Number(value);
    const bounded = Number.isFinite(percentage)
        ? Math.max(0, Math.min(100, percentage))
        : 50;
    return bounded * .02;
}

export const ROOT_PARENT_SENTINEL = 0;
export const RICEWM_TASKBAR_ID = 0xe0000001;
export const RICEWM_GNOME_PANEL_ID = 0xe0000008;
export const RICEWM_GNOME_BACKGROUND_ID = 0xe000000c;
export const RICEWM_MENU_ID = 0xe0000002;
export const RICEWM_NOTIFICATION_ID = 0xe0000004;
export const RICEWM_TASK_PREVIEW_ID = 0xe0000006;
export const RICEWM_NATIVE_PREVIEW_PREFIX = 0x80000000;
export const RICEWM_FIRST_MANAGED_FRAME_ID = 0xe1000000;
export const RICEWM_MANAGED_FRAME_ID_LIMIT = 0xe2000000;
export const MAX_NATIVE_MEDIA_SURFACES = 16_384;
export const MAX_NATIVE_MEDIA_REQUEST_TRACKING = 16_384;
export const RICE_NATIVE_MENU_ICONS = new Map(NATIVE_ENTRYPOINTS.map(entrypoint => [
    nativeEntrypointTitle(entrypoint),
    nativeEntrypointIconName(entrypoint),
]));
export const RICE_BUILTIN_MENU_ICONS = new Map([
    ["Applications", "view-grid"],
    ["Programs", "program"],
    ["Run...", "terminal"],
    ["Themes", "image"],
    ["Settings", "settings"],
    ["Windows Update", "windows-update"],
    ["Favorites", "folder"],
    ["Documents", "documents"],
    ["Find", "search"],
    ["Help", "help"],
    ["Log Off...", "program"],
    ["Shut Down...", "window-close"],
    ["Accessories", "folder"],
    ["Development", "view-grid"],
    ["Games", "selection-check"],
    ["Graphics", "image"],
    ["Internet", "network"],
    ["Multimedia", "media-play"],
    ["Office", "file"],
    ["Settings & System", "settings"],
    ["Other", "folder"],
    ["Launch", "media-play"],
    ["Open containing folder", "folder"],
    ["Open Menu Directory", "folder"],
    ["Restore windows", "window-restore"],
    ["Next workspace", "chevron-right"],
    ["Previous workspace", "chevron-left"],
    ["Previous", "chevron-left"],
    ["Next", "chevron-right"],
    ["Workspaces", "desktop"],
    ["Show desktop", "desktop"],
    ["Toolbar preferences", "settings"],
    ["Adjust quick shortcuts", "settings"],
    ["Customize", "settings"],
    ["Exit RiceWM", "logout"],
    ["Close window", "window-close"],
    ["Close", "window-close"],
    ["Maximize", "window-maximize"],
    ["Minimize", "window-minimize"],
    ["Restore", "window-restore"],
    ["Close entire stack", "window-close"],
    ["Tile windows", "view-grid"],
    ["Tile stack", "view-grid"],
    ["Cascade windows", "view-detail"],
    ["Cascade stack", "view-detail"],
    ["No applications", "dialog-information"],
    ["System Monitor", "system-monitor"],
    ["Monitor", "system-monitor"],
]);

export function riceNativeMenuIconName(title) {
    const label = String(title || "");
    return RICE_NATIVE_MENU_ICONS.get(label) ||
        RICE_BUILTIN_MENU_ICONS.get(label) || null;
}

export function riceNativeTaskIconName(title) {
    const label = String(title || "").replace(/\s+\(\d+\)$/u, "");
    if (label === "RiceWM" || label === "Start") return "ricewm-logo";
    for (const [application, icon] of RICE_NATIVE_MENU_ICONS) {
        if (label === application || label.endsWith(` - ${application}`)) return icon;
    }
    return null;
}

export function appendWedgedSurfacePath(context, rect) {
    const cut = Math.max(7, Math.min(
        16,
        Math.floor(Math.min(rect.width, rect.height) * .08),
    ));
    context.moveTo(rect.x + cut, rect.y);
    context.lineTo(rect.x + rect.width, rect.y);
    context.lineTo(rect.x + rect.width, rect.y + rect.height - cut);
    context.lineTo(rect.x + rect.width - cut, rect.y + rect.height);
    context.lineTo(rect.x, rect.y + rect.height);
    context.lineTo(rect.x, rect.y + cut);
    context.closePath();
}

export function defaultRequestFrame(callback) {
    if (typeof globalThis.requestAnimationFrame === "function") {
        return globalThis.requestAnimationFrame(callback);
    }
    return globalThis.setTimeout(callback, 0);
}

export function defaultCancelFrame(handle) {
    if (typeof globalThis.cancelAnimationFrame === "function") {
        globalThis.cancelAnimationFrame(handle);
    } else {
        globalThis.clearTimeout(handle);
    }
}

export function pixelColor(pixel) {
    return `#${(pixel & 0x00ff_ffff).toString(16).padStart(6, "0")}`;
}

export function sameSceneWindow(left, right) {
    if (!left || !right) return false;
    return [
        "id", "depth", "parent", "x", "y", "width", "height",
        "borderWidth", "stackIndex", "ownedWindow", "resizableWindow",
        "backgroundPixel", "viewable", "overrideRedirect", "inputOnly",
        "desktopLayer", "shellLayer", "backdropMaterial", "activeWindow",
        "maximizedWindow",
    ].every(key => left[key] === right[key]);
}

export function overviewEase(progress) {
    const remaining = 1 - Math.max(0, Math.min(1, progress));
    return 1 - remaining * remaining * remaining;
}

export function riceWindowTransitionOrigin(window, menu) {
    return Object.freeze({
        x: window.x + window.width / 2,
        // The main RiceWM launcher is attached to the bottom panel and folds
        // upward. Override-redirect application menus remain attached to
        // their top edge and fold downward.
        y: window.id === RICEWM_MENU_ID
            ? window.y + window.height
            : menu
                ? window.y
                : window.y + window.height / 2,
    });
}

export function riceOverviewLayout(items, width, height, minimumTop = 0) {
    const windows = Array.isArray(items) ? items : [];
    if (windows.length === 0) return Object.freeze([]);
    const viewportWidth = Math.max(1, Number(width) || 1);
    const viewportHeight = Math.max(1, Number(height) || 1);
    const side = Math.min(72, Math.max(20, viewportWidth * 0.055));
    const top = Math.max(
        Math.min(88, Math.max(56, viewportHeight * 0.1)),
        Math.max(0, Number(minimumTop) || 0),
    );
    const bottom = Math.min(72, Math.max(44, viewportHeight * 0.08));
    const gap = Math.min(30, Math.max(14, viewportWidth * 0.022));
    const availableWidth = Math.max(1, viewportWidth - side * 2);
    const availableHeight = Math.max(1, viewportHeight - top - bottom);
    const columns = Math.max(1, Math.min(
        windows.length,
        Math.ceil(Math.sqrt(
            windows.length * availableWidth / Math.max(1, availableHeight),
        )),
    ));
    const rows = Math.ceil(windows.length / columns);
    const cellWidth = (availableWidth - gap * (columns - 1)) / columns;
    const cellHeight = (availableHeight - gap * (rows - 1)) / rows;
    return Object.freeze(windows.map((item, index) => {
        const column = index % columns;
        const row = Math.floor(index / columns);
        const sourceWidth = Math.max(1, Number(item?.source?.width) || 1);
        const sourceHeight = Math.max(1, Number(item?.source?.height) || 1);
        const maximumWidth = Math.max(1, cellWidth - 12);
        const maximumHeight = Math.max(1, cellHeight - 34);
        const scale = Math.min(
            maximumWidth / sourceWidth,
            maximumHeight / sourceHeight,
            1.2,
        );
        const previewWidth = sourceWidth * scale;
        const previewHeight = sourceHeight * scale;
        const cellX = side + column * (cellWidth + gap);
        const cellY = top + row * (cellHeight + gap);
        const preview = Object.freeze({
            x: cellX + (cellWidth - previewWidth) / 2,
            y: cellY + (maximumHeight - previewHeight) / 2,
            width: previewWidth,
            height: previewHeight,
        });
        return Object.freeze({
            item,
            preview,
            hit: Object.freeze({
                x: cellX,
                y: cellY,
                width: cellWidth,
                height: cellHeight,
            }),
            labelY: cellY + maximumHeight + 17,
        });
    }));
}

export function riceFocusOverlayRects(viewport, activeFrame) {
    const width = Math.max(0, Number(viewport?.width) || 0);
    const height = Math.max(0, Number(viewport?.height) || 0);
    if (width === 0 || height === 0 || !activeFrame) return Object.freeze([]);
    const left = Math.max(0, Math.min(width, Number(activeFrame.x) || 0));
    const top = Math.max(0, Math.min(height, Number(activeFrame.y) || 0));
    const right = Math.max(left, Math.min(
        width,
        (Number(activeFrame.x) || 0) + Math.max(0, Number(activeFrame.width) || 0),
    ));
    const bottom = Math.max(top, Math.min(
        height,
        (Number(activeFrame.y) || 0) + Math.max(0, Number(activeFrame.height) || 0),
    ));
    return Object.freeze([
        { x: 0, y: 0, width, height: top },
        { x: 0, y: top, width: left, height: bottom - top },
        { x: right, y: top, width: width - right, height: bottom - top },
        { x: 0, y: bottom, width, height: height - bottom },
    ].filter(rect => rect.width > 0 && rect.height > 0)
        .map(rect => Object.freeze(rect)));
}

export function defaultCanvasFactory(width, height) {
    if (typeof globalThis.OffscreenCanvas === "function") {
        return new globalThis.OffscreenCanvas(width, height);
    }
    if (globalThis.document?.createElement) {
        const canvas = globalThis.document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        return canvas;
    }
    throw new TypeError("RiceWM cannot allocate a retained drawable canvas");
}


export {
    appendWindowSilhouette, RiceNativeCanvasWindow,
    FrostedIceMaterial, RiceGnomeEffectRenderer, riceIconName,
    NATIVE_ENTRYPOINTS, nativeEntrypointIconName, nativeEntrypointTitle,
    nativeTitlebarLayout,
};
