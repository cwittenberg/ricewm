import { FrostedIceMaterial } from "./rice-frosted-ice.mjs";

const THICKNESS = 32;
const PANEL_THICKNESS = 32;
const HIDDEN_THICKNESS = 4;
const BUTTON_SIZE = 26;
const BUTTON_STEP = 30;
const MATERIALS = new WeakMap();

export function riceNativeDockSnapPosition(x, y, root) {
    const width = Math.max(1, Math.floor(Number(root?.width) || 0));
    const height = Math.max(1, Math.floor(Number(root?.height) || 0));
    const pointerX = Math.max(0, Math.min(width, Math.floor(Number(x) || 0)));
    const pointerY = Math.max(0, Math.min(height, Math.floor(Number(y) || 0)));
    return [
        [height - pointerY, "Bottom"],
        [pointerY, "Top"],
        [pointerX, "Left"],
        [width - pointerX, "Right"],
    ].reduce((nearest, candidate) =>
        candidate[0] < nearest[0] ? candidate : nearest
    )[1];
}

function materialFor(context) {
    let material = MATERIALS.get(context);
    if (!material) {
        material = new FrostedIceMaterial();
        MATERIALS.set(context, material);
    }
    return material;
}

export function riceNativeDockWorkArea(shell, root) {
    const width = Math.max(0, Math.floor(Number(root?.width) || 0));
    const height = Math.max(0, Math.floor(Number(root?.height) || 0));
    const inset = 0;
    const workArea = { x: inset, y: inset, width: Math.max(0,width-inset*2), height: Math.max(0,height-inset*2) };
    if (shell?.gnomePanelVisible) {
        const reserved = Math.min(PANEL_THICKNESS, workArea.height);
        workArea.y += reserved;
        workArea.height -= reserved;
    }
    if (!shell?.dockVisible || shell.dockAutoHide) return workArea;

    // The compact dock floats above maximized content. Its transparent
    // compositor allocation must not turn into an invisible 88px work-area.
    const reservedThickness = shell.dockStyle === "Compact dock" ? 0 : THICKNESS;
    switch (shell.dockPosition) {
        case "Top": {
            const reserved = Math.min(reservedThickness, workArea.height);
            workArea.y += reserved;
            workArea.height -= reserved;
            break;
        }
        case "Left": {
            const reserved = Math.min(reservedThickness, width);
            workArea.x += reserved;
            workArea.width -= reserved;
            break;
        }
        case "Right":
            workArea.width -= Math.min(reservedThickness, width);
            break;
        default:
            workArea.height -= Math.min(reservedThickness, height);
    }
    return workArea;
}

export function riceNativeDockLayout(shell, windows, root) {
    const thickness = shell.dockAutoHide && !shell.dockRevealed
        ? HIDDEN_THICKNESS
        : THICKNESS;
    const panelTop = shell?.gnomePanelVisible
        ? Math.min(PANEL_THICKNESS, root.height)
        : 0;
    const availableHeight = Math.max(0, root.height - panelTop);
    const extent = Math.min(
        Math.max(12 + Math.min(windows.length, 16) * BUTTON_STEP, 50),
        Math.max(root.width, availableHeight),
    );
    let rect;
    switch (shell.dockPosition) {
        case "Top":
            rect = {
                x: Math.max(0, Math.floor((root.width - extent) / 2)),
                y: panelTop,
                width: Math.min(extent, root.width),
                height: Math.min(thickness, availableHeight),
            };
            break;
        case "Left":
            rect = {
                x: 0,
                y: panelTop + Math.max(
                    0,
                    Math.floor((availableHeight - extent) / 2),
                ),
                width: thickness,
                height: Math.min(extent, availableHeight),
            };
            break;
        case "Right":
            rect = {
                x: root.width - thickness,
                y: panelTop + Math.max(
                    0,
                    Math.floor((availableHeight - extent) / 2),
                ),
                width: thickness,
                height: Math.min(extent, availableHeight),
            };
            break;
        default:
            rect = {
                x: Math.max(0, Math.floor((root.width - extent) / 2)),
                y: root.height - thickness,
                width: Math.min(extent, root.width),
                height: thickness,
            };
    }
    const horizontal = rect.width > rect.height;
    const buttons = shell.dockAutoHide && !shell.dockRevealed
        ? []
        : windows.slice(0, 16).map((window, index) => ({
            app: String(window.app),
            rect: {
                x: rect.x + (horizontal ? 6 + index * BUTTON_STEP : 3),
                y: rect.y + (horizontal ? 3 : 6 + index * BUTTON_STEP),
                width: BUTTON_SIZE,
                height: BUTTON_SIZE,
            },
            window,
        }));
    return { rect, buttons };
}

export function renderRiceNativeDock(
    context,
    shell,
    windows,
    root,
    iconRenderer = null,
    glassMaterial = null,
) {
    if (!shell?.dockVisible) return;
    const entries = (shell.dockEntries || windows).filter(
        window => window.role !== "desktop",
    );
    if (entries.length === 0) return;
    const { rect, buttons } = riceNativeDockLayout(shell, entries, root);
    if (riceNativeFullscreenCoversRect(windows.findLast(window => window.active), rect)) return;
    context.save();
    const frosted = shell.frostedIce?.enabled && shell.frostedIce?.panels &&
        !shell.highContrast && context.canvas;
    if (frosted) {
        (glassMaterial || materialFor(context)).draw(
            context,
            context.canvas,
            rect,
            12,
            shell.frostedIce,
            globalThis.performance?.now?.() || Date.now(),
        );
    } else {
        context.fillStyle = shell.highContrast
            ? "rgba(0, 0, 0, 0.98)"
            : "rgba(25, 35, 48, 0.94)";
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
    }
    if (!shell.dockAutoHide || shell.dockRevealed) {
        buttons.forEach(button => {
            const { window } = button;
            const { x, y, width, height } = button.rect;
            context.fillStyle = window.minimized
                ? "rgba(255,255,255,.08)"
                : window.role === "desktop"
                ? "rgba(255,255,255,.12)"
                : "rgba(255,255,255,.2)";
            context.fillRect(x, y, width, height);
            if (iconRenderer) {
                iconRenderer.render({
                    icon: dockIcon(window),
                    color: [255, 255, 255, 255],
                    rect: {
                        x: x + 4,
                        y: y + 4,
                        width: width - 8,
                        height: height - 8,
                    },
                });
            } else {
                drawDockGlyph(context, window, x, y, width, height);
            }
        });
    }
    if (shell.dockPreviewPosition) {
        const preview = riceNativeDockLayout({
            ...shell,
            dockPosition: shell.dockPreviewPosition,
            dockAutoHide: false,
            dockRevealed: true,
        }, entries, root).rect;
        context.strokeStyle = "rgba(255,255,255,.9)";
        context.lineWidth = 2;
        context.setLineDash?.([4, 3]);
        context.strokeRect?.(
            preview.x + 1,
            preview.y + 1,
            Math.max(0, preview.width - 2),
            Math.max(0, preview.height - 2),
        );
        context.setLineDash?.([]);
    }
    context.restore();
}

/** Fullscreen only obscures chrome on the output occupied by its owner. */
export function riceNativeFullscreenCoversRect(window, rect) {
    const frame = window?.frame;
    return Boolean(window?.fullscreen && !window.minimized && frame && rect &&
        rect.x >= frame.x && rect.y >= frame.y &&
        rect.x + rect.width <= frame.x + frame.width && rect.y + rect.height <= frame.y + frame.height);
}

function dockIcon(window) {
    const app = String(window.app || "").toLowerCase();
    const title = String(window.title || "").toLowerCase();
    if (app.includes("calc") || title.includes("calc")) return 60;
    if (app.includes("paper") || title.includes("paper") || title.includes("text")) {
        return 61;
    }
    if (app.includes("file") || title.includes("file")) return 62;
    if (
        app.includes("view") ||
        title.includes("view") ||
        title.includes("image")
    ) {
        return 63;
    }
    return 1;
}

function drawDockGlyph(context, window, x, y, width, height) {
    const title = String(window.title || "").toLowerCase();
    context.strokeStyle = "white";
    context.fillStyle = "white";
    context.lineWidth = 2;
    if (title.includes("calc")) {
        context.strokeRect(x + 8, y + 5, width - 16, height - 10);
        context.fillRect(x + 12, y + 9, width - 24, 5);
        for (let row = 0; row < 2; row += 1) {
            for (let column = 0; column < 3; column += 1) {
                context.fillRect(x + 12 + column * 5, y + 18 + row * 5, 3, 3);
            }
        }
    } else if (title.includes("file")) {
        context.strokeRect(x + 8, y + 10, width - 16, height - 18);
        context.beginPath();
        context.moveTo(x + 9, y + 10);
        context.lineTo(x + 15, y + 6);
        context.lineTo(x + 23, y + 6);
        context.stroke();
    } else if (title.includes("paper") || title.includes("text")) {
        context.strokeRect(x + 9, y + 6, width - 18, height - 12);
        for (let row = 0; row < 3; row += 1) {
            context.fillRect(x + 13, y + 12 + row * 5, width - 26, 2);
        }
    } else if (title.includes("view") || title.includes("image")) {
        context.strokeRect(x + 7, y + 8, width - 14, height - 16);
        context.beginPath();
        context.moveTo(x + 10, y + height - 11);
        context.lineTo(x + 17, y + 17);
        context.lineTo(x + 22, y + 22);
        context.lineTo(x + width - 10, y + 13);
        context.stroke();
    } else {
        for (let row = 0; row < 2; row += 1) {
            for (let column = 0; column < 2; column += 1) {
                context.beginPath();
                context.arc(x + 13 + column * 9, y + 13 + row * 9, 3, 0, Math.PI * 2);
                context.fill();
            }
        }
    }
}
