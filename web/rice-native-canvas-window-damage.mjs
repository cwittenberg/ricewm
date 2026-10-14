export function adaptiveStructuralInk(source, rect, tint, fallback) {
    const fallbackInk = Array.isArray(fallback)
        ? fallback.slice()
        : [255, 255, 255, 255];
    const width = Math.max(0, Math.min(
        Math.floor(Number(rect?.width) || 0),
        Math.floor(Number(source?.width) || 0) - Math.floor(Number(rect?.x) || 0),
    ));
    const height = Math.max(0, Math.min(
        Math.floor(Number(rect?.height) || 0),
        Math.floor(Number(source?.height) || 0) - Math.floor(Number(rect?.y) || 0),
    ));
    const x = Math.max(0, Math.floor(Number(rect?.x) || 0));
    const y = Math.max(0, Math.floor(Number(rect?.y) || 0));
    if (width < 1 || height < 1) return fallbackInk;
    try {
        const pixels = source.getContext?.("2d", { willReadFrequently: true })
            ?.getImageData?.(x, y, width, height)?.data;
        if (!pixels || pixels.length < width * height * 4) return fallbackInk;
        const stride = Math.max(1, Math.floor(Math.sqrt(width * height / 256)));
        let red = 0;
        let green = 0;
        let blue = 0;
        let samples = 0;
        for (let sampleY = 0; sampleY < height; sampleY += stride) {
            for (let sampleX = 0; sampleX < width; sampleX += stride) {
                const offset = (sampleY * width + sampleX) * 4;
                red += pixels[offset];
                green += pixels[offset + 1];
                blue += pixels[offset + 2];
                samples += 1;
            }
        }
        if (samples < 1) return fallbackInk;
        const tintAlpha = Array.isArray(tint) && tint.length >= 4
            ? Math.max(0, Math.min(1, Number(tint[3]) / 255))
            : .24;
        const composite = (backdrop, index) => backdrop / samples * (1 - tintAlpha) +
            (Number(tint?.[index]) || 0) * tintAlpha;
        const linear = channel => {
            channel /= 255;
            return channel <= .04045
                ? channel / 12.92
                : ((channel + .055) / 1.055) ** 2.4;
        };
        const luminance = linear(composite(red, 0)) * .2126 +
            linear(composite(green, 1)) * .7152 +
            linear(composite(blue, 2)) * .0722;
        return luminance >= .179129
            ? [10, 8, 13, fallbackInk[3] ?? 255]
            : [255, 255, 255, fallbackInk[3] ?? 255];
    } catch {
        // A protected or unavailable backing canvas must retain the theme's
        // explicit fallback instead of faulting the entire native window.
        return fallbackInk;
    }
}

export function sameRect(left, right) {
    return left?.x === right?.x && left?.y === right?.y &&
        left?.width === right?.width && left?.height === right?.height;
}

export function popupSurfaceRects(commands, fallback) {
    const panes = [];
    for (let index = 0; index < (commands?.length || 0); index += 1) {
        const fill = commands[index];
        const border = commands[index + 1];
        const glassPane = fill?.type === "glass" && fill.popup === true;
        const solidPane = fill?.type === "fill" && border?.type === "border" &&
            sameRect(fill.rect, border.rect);
        if (
            (!glassPane && !solidPane) ||
            panes.some(pane => sameRect(pane, fill.rect))
        ) {
            continue;
        }
        panes.push(fill.rect);
    }
    return panes.length > 0 ? panes : [fallback];
}

function sameArray(left, right) {
    if (left === right) return true;
    if (!left || !right || left.length !== right.length) return false;
    for (let index = 0; index < left.length; index += 1) {
        // Snapshot transport rebuilds dock records and terminal cell tuples.
        // Compare their values, not their allocation identity: otherwise an
        // unchanged desktop refresh invalidates every open window.
        if (!sameValue(left[index], right[index])) return false;
    }
    return true;
}

function sameValue(left, right) {
    if (left === right) return true;
    if (Array.isArray(left) || ArrayBuffer.isView(left)) {
        return sameArray(left, right);
    }
    if (!left || !right || typeof left !== "object" || typeof right !== "object") {
        return false;
    }
    const keys = new Set([...Object.keys(left), ...Object.keys(right)]);
    for (const key of keys) {
        if (!sameValue(left[key], right[key])) return false;
    }
    return true;
}

function sameCommand(left, right) {
    if (left === right) return true;
    if (!left || !right || left.type !== right.type || !sameRect(left.rect, right.rect)) {
        return false;
    }
    const keys = new Set([...Object.keys(left), ...Object.keys(right)]);
    keys.delete("rect");
    keys.delete("displayList");
    for (const key of keys) {
        const a = left[key];
        const b = right[key];
        if (Array.isArray(a) || ArrayBuffer.isView(a)) {
            if (!sameArray(a, b)) return false;
        } else if (a && typeof a === "object") {
            if (!sameValue(a, b)) return false;
        } else if (a !== b) {
            return false;
        }
    }
    if (left.type !== 'web-scene' || left.displayList === right.displayList) return true;
    // Structured cloning recreates these bytes on unrelated desktop updates.
    // Equal content is not damage; still detect changed bytes at the same epoch.
    const a = left.displayList, b = right.displayList;
    if (!a || !b || !Number.isSafeInteger(a.length) || a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
    return true;
}

export function globalCommandRect(native, command, contentDamage = true) {
    let rect = command?.rect;
    if (!rect || rect.width <= 0 || rect.height <= 0) return null;
    if (contentDamage && command.type === "web-scene" && command.damageRect) {
        const damage = command.damageRect;
        if (damage.width <= 0 || damage.height <= 0) return null;
        const surface = rect;
        const scale = surface.width / Math.max(1, command.viewportWidth);
        const damaged = {
            x: surface.x + damage.x * scale,
            y: surface.y + damage.y * scale,
            width: damage.width * scale,
            height: damage.height * scale,
        };
        const right = Math.min(surface.x + surface.width, damaged.x + damaged.width);
        const bottom = Math.min(surface.y + surface.height, damaged.y + damaged.height);
        rect = {
            x: Math.max(surface.x, damaged.x),
            y: Math.max(surface.y, damaged.y),
            width: Math.max(0, right - Math.max(surface.x, damaged.x)),
            height: Math.max(0, bottom - Math.max(surface.y, damaged.y)),
        };
        if (rect.width <= 0 || rect.height <= 0) return null;
    }
    return {
        x: native.frame.x + rect.x,
        y: native.frame.y + (native.role === "desktop" ? 0 : native.frame.titleHeight) + rect.y,
        width: rect.width,
        height: rect.height,
    };
}

export function frameRect(native) {
    return native?.frame ? {
        x: native.frame.x,
        y: native.frame.y,
        width: native.frame.width,
        height: native.frame.height,
    } : null;
}

// Canvas shadows are output pixels even though they are outside the logical
// window frame.  A move/resize/activation repaint that clips exactly to the
// frame leaves the old shadow in the retained composition, where glass can
// sample it again on later frames.  Keep this comfortably above the current
// 28 px shadow blur plus its 10 px vertical offset.
const NATIVE_WINDOW_VISUAL_OUTSET = 48;

function visualFrameRect(native) {
    const rect = frameRect(native);
    if (!rect || native?.role === "desktop") return rect;
    const dialog = native.dialog?.title?.startsWith('panel:drawer-dialog:') && native.dialog.rect;
    const pane = dialog && {x:native.frame.x+dialog.x, y:native.frame.y+native.frame.titleHeight+dialog.y,
        width:dialog.width,height:dialog.height};
    const bounds = pane ? {x:Math.min(rect.x,pane.x), y:Math.min(rect.y,pane.y),
        width:Math.max(rect.x+rect.width,pane.x+pane.width)-Math.min(rect.x,pane.x),
        height:Math.max(rect.y+rect.height,pane.y+pane.height)-Math.min(rect.y,pane.y)} : rect;
    return expandedRect(bounds, NATIVE_WINDOW_VISUAL_OUTSET);
}

function intersects(left, right) {
    return left && right &&
        left.x < right.x + right.width &&
        left.y < right.y + right.height &&
        left.x + left.width > right.x &&
        left.y + left.height > right.y;
}

export function expandedRect(rect, amount = 0) {
    if (!rect || rect.width <= 0 || rect.height <= 0) return null;
    return {
        x: rect.x - amount,
        y: rect.y - amount,
        width: rect.width + amount * 2,
        height: rect.height + amount * 2,
    };
}

export function riceNativeCommandIntersectsDamage(native, command, damageRects) {
    // A web scene's damageRect reports its latest content change, not where
    // its retained display list paints. Uncover, cursor and animated-terminal
    // damage must replay every overlapping plane, even after an empty page
    // update. Only snapshot invalidation uses the narrower content damage.
    return !Array.isArray(damageRects) || damageRects.some(damage =>
        intersects(damage, globalCommandRect(native, command, false))
    );
}

// A retained portal only needs a new backdrop when a visible part changes.
// Rectangles covered by a later opaque plane are not visible backdrop damage.
// Treat partial/combined coverage conservatively: an unnecessary copy is safe,
// whereas dropping a single exposed pixel would leave stale content.
export function riceNativeBackdropIntersectsDamage(rect, damageRects, occlusions = []) {
    if (!Array.isArray(damageRects)) return true;
    return damageRects.some(damage => {
        if (!intersects(rect, damage)) return false;
        const x = Math.max(rect.x, damage.x), y = Math.max(rect.y, damage.y);
        const right = Math.min(rect.x + rect.width, damage.x + damage.width);
        const bottom = Math.min(rect.y + rect.height, damage.y + damage.height);
        return !occlusions.some(cover => cover.x <= x && cover.y <= y &&
            cover.x + cover.width >= right && cover.y + cover.height >= bottom);
    });
}

// Only a settled, solid window interior can hide an animated lower image.
// Keep glass, portals, custom silhouettes and inactive translucency live.
export function riceNativeOpaqueInterior(native) {
    const { frame, chrome, shell } = native || {};
    if (!frame || !chrome || native.layerVisible === false || native.minimized ||
        native.role === 'desktop' || native.dialogOnly || native.tileClip ||
        (!native.active && shell?.inactiveWindowsTranslucent !== false) ||
        chrome.material === 'frosted-ice' || chrome.body?.[3] !== 255 ||
        (shell?.surface?.frame && shell.surface.frame !== 'standard') ||
        native.commands?.some(c => c.type === 'host-portal' || c.type === 'terminal-portal' ||
            c.type === 'terminal-grid' || (c.type === 'web-scene' && !c.vectorScene))) return null;
    const inset = Math.max(2, Number(chrome.radius) || 0);
    const rect = { x: frame.x + inset, y: frame.y + frame.titleHeight + inset,
        width: frame.width - inset * 2, height: frame.height - frame.titleHeight - inset * 2 };
    return rect.width > 0 && rect.height > 0 ? rect : null;
}

export function riceNativeSnapshotDamage(previous, next) {
    const damage = [];
    const before = new Map(previous.map(native => [native.app, native]));
    const after = new Map(next.map(native => [native.app, native]));
    for (const app of new Set([...before.keys(), ...after.keys()])) {
        const oldNative = before.get(app);
        const newNative = after.get(app);
        const geometryChanged = !oldNative || !newNative ||
            !sameRect(oldNative.frame, newNative.frame);
        const outsideFrameChanged = geometryChanged ||
            ((oldNative?.dialog?.title?.startsWith("panel:drawer-dialog:") || newNative?.dialog?.title?.startsWith("panel:drawer-dialog:")) && !sameValue(oldNative?.dialog, newNative?.dialog)) ||
            oldNative?.active !== newNative?.active ||
            oldNative?.maximized !== newNative?.maximized ||
            !sameValue(oldNative?.chrome, newNative?.chrome) ||
            !sameValue(oldNative?.shell, newNative?.shell);
        if (!oldNative || !newNative ||
            geometryChanged ||
            oldNative.frame?.titleHeight !== newNative.frame?.titleHeight ||
            oldNative.frame?.contentHeight !== newNative.frame?.contentHeight ||
            oldNative.active !== newNative.active ||
            oldNative.layerVisible !== newNative.layerVisible ||
            oldNative.maximized !== newNative.maximized ||
            oldNative.title !== newNative.title ||
            !sameValue(oldNative.desktopBackground, newNative.desktopBackground) ||
            // Dialog and popup surfaces include compositor-owned dimming,
            // shadows, and clipping outside the retained command records.
            // When either surface appears, disappears, or moves, command-by-
            // command damage cannot erase those pixels reliably.  Repaint the
            // owning frame so closing a desktop rename/link dialog never
            // leaves a stale horizontal strip above the taskbar.
            !sameValue(oldNative.dialog, newNative.dialog) ||
            !sameValue(oldNative.popup, newNative.popup) ||
            !sameValue(oldNative.chrome, newNative.chrome) ||
            !sameValue(oldNative.shell, newNative.shell)
        ) {
            const oldRect = outsideFrameChanged
                ? visualFrameRect(oldNative) : frameRect(oldNative);
            const newRect = outsideFrameChanged
                ? visualFrameRect(newNative) : frameRect(newNative);
            if (oldRect) damage.push(oldRect);
            if (newRect) damage.push(newRect);
            continue;
        }
        const oldCommands = [
            ...(oldNative.commands || []),
            ...(oldNative.popupCommands || []),
        ];
        const newCommands = [
            ...(newNative.commands || []),
            ...(newNative.popupCommands || []),
        ];
        const count = Math.max(oldCommands.length, newCommands.length);
        for (let index = 0; index < count; index += 1) {
            if (sameCommand(oldCommands[index], newCommands[index])) continue;
            if (
                oldCommands[index]?.type === "web-scene" &&
                newCommands[index]?.type === "web-scene" &&
                oldCommands[index].surface === newCommands[index].surface &&
                sameRect(oldCommands[index].rect, newCommands[index].rect) &&
                newCommands[index].damageRect !== null
            ) {
                const rect = globalCommandRect(newNative, newCommands[index]);
                if (rect) damage.push(rect);
                continue;
            }
            const oldRect = globalCommandRect(oldNative, oldCommands[index]);
            const newRect = globalCommandRect(newNative, newCommands[index]);
            if (oldRect) damage.push(oldRect);
            if (newRect) damage.push(newRect);
        }
    }
    return damage;
}
